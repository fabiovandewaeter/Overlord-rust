use std::collections::HashSet;

use crate::{
    UpsCounter,
    items::Inventory,
    map::{
        StructureManager, TILE_SIZE, get_neighbors, is_tile_passable, rounded_tile_pos_to_world,
        world_pos_to_rounded_tile,
    },
    // pathfinding::PathfindingAgent,
    // units::tasks::{ActionQueue, CurrentAction, CurrentTask},
};
use bevy::prelude::*;

pub const UNIT_REACH: u8 = 1;
// pub const UNIT_DEFAULT_MOVEMENT_SPEED: f32 = 5.0;
pub const UNIT_DEFAULT_MOVEMENT_SPEED: f32 = 2.0; // 2 tiles per second
// pub const UNIT_DEFAULT_ROTATION_SPEED: f32 = f32::to_radians(360.0);

#[derive(Component, Debug, Default)]
#[require(
    Sprite,
    Transform,
    TileMovement,
    // PathfindingAgent,
    Inventory,
    // ActionQueue,
    // CurrentAction,
    // CurrentTask
)]
pub struct Unit {
    pub name: String,
}

#[derive(Component)]
pub struct TileMovement {
    pub desired_target_tile: Option<IVec2>,
    pub movement_speed: f32,
    pub move_timer: Timer,
}

impl Default for TileMovement {
    fn default() -> Self {
        Self {
            desired_target_tile: None,
            movement_speed: UNIT_DEFAULT_MOVEMENT_SPEED,
            move_timer: Timer::from_seconds(1.0 / UNIT_DEFAULT_MOVEMENT_SPEED, TimerMode::Once),
        }
    }
}

impl TileMovement {
    pub fn new(movement_speed: f32) -> Self {
        let frequency = if movement_speed > 0.0 {
            1.0 / movement_speed
        } else {
            1000.0
        };
        Self {
            desired_target_tile: None,
            movement_speed,
            move_timer: Timer::from_seconds(frequency, TimerMode::Once), // 0.5 sec par case par défaut
        }
    }

    pub fn update_speed(&mut self, new_speed: f32) {
        self.movement_speed = new_speed;
        let frequency = if self.movement_speed > 0.0 {
            1.0 / self.movement_speed
        } else {
            1000.0
        };
        self.move_timer = Timer::from_seconds(frequency, TimerMode::Once);
    }

    pub fn reset_timer(&mut self) {
        let frequency = if self.movement_speed > 0.0 {
            1.0 / self.movement_speed
        } else {
            1000.0
        };
        self.move_timer = Timer::from_seconds(frequency, TimerMode::Once);
    }
}

/// to add if the entity needs to checks its collisions with other entities (collisions with walls isn't affected)
#[derive(Component)]
pub struct UnitUnitCollisions;

pub fn move_and_collide_units_system(
    structure_manager: Res<StructureManager>,
    // occupied_query: Query<&Transform, (With<Unit>, With<UnitUnitCollisions>)>,
    mut unit_query: Query<
        (
            &mut Transform,
            &mut TileMovement,
            Option<&UnitUnitCollisions>,
        ),
        With<Unit>,
    >,
    time: Res<Time>,
) {
    // all tiles occupied by units
    let mut occupied_tiles: HashSet<IVec2> = HashSet::new();
    for (transform, _, unit_unit_collisions) in unit_query.iter() {
        if unit_unit_collisions.is_some() {
            let tile = world_pos_to_rounded_tile(transform.translation.xy());
            occupied_tiles.insert(tile);
        }
    }

    for (mut transform, mut tile_movement, unit_unit_collisions) in unit_query.iter_mut() {
        tile_movement.move_timer.tick(time.delta());
        if let Some(desired_target_tile) = tile_movement.desired_target_tile {
            if tile_movement.move_timer.finished() {
                // if there is a structure
                if !is_tile_passable(desired_target_tile, &structure_manager) {
                    tile_movement.desired_target_tile = None;
                    continue;
                }

                // if there is a unit with collisions and current unit has collisions too
                if occupied_tiles.contains(&desired_target_tile) && unit_unit_collisions.is_some() {
                    tile_movement.desired_target_tile = None;
                    continue;
                }

                let target_world_pos = rounded_tile_pos_to_world(desired_target_tile);
                transform.translation.x = target_world_pos.x;
                transform.translation.y = target_world_pos.y;

                tile_movement.desired_target_tile = None;
            }
        }
    }
}

// pub fn display_units_with_no_current_action_system(unit_query: Query<&CurrentAction, With<Unit>>) {
//     let mut counter = 0;
//     for current_action in unit_query.iter() {
//         if current_action.action.is_none() {
//             counter += 1;
//         }
//     }
//     println!("Counter units with no current action: {}", counter);
// }

pub fn display_units_inventory_system(unit_query: Query<&Inventory>) {
    for inventory in unit_query.iter() {
        if !inventory.stackable_items.is_empty() {
            println!("{:?}", inventory);
        }
    }
}

pub fn test_units_control_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut unit_query: Query<(&Transform, &mut TileMovement), With<Unit>>,
    time: Res<Time>,
) {
    for (transform, mut tile_movement) in unit_query.iter_mut() {
        if tile_movement.desired_target_tile.is_some() {
            continue;
        }

        let mut delta = IVec2::new(0, 0);
        if keyboard_input.pressed(KeyCode::ArrowLeft) {
            // delta.x -= tile_movement.movement_speed as i32;
            delta.x -= 1;
        }

        if keyboard_input.pressed(KeyCode::ArrowRight) {
            // delta.x += tile_movement.movement_speed as i32;
            delta.x += 1;
        }

        if keyboard_input.pressed(KeyCode::ArrowUp) {
            // delta.y += tile_movement.movement_speed as i32;
            delta.y += 1;
        }

        if keyboard_input.pressed(KeyCode::ArrowDown) {
            // delta.y -= tile_movement.movement_speed as i32;
            delta.y -= 1;
        }

        if delta == IVec2::ZERO {
            continue;
        }

        let current_tile = world_pos_to_rounded_tile(transform.translation.xy());
        let target_tile = current_tile + delta;

        tile_movement.desired_target_tile = Some(target_tile);
        tile_movement.reset_timer();
    }
}

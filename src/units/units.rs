use std::collections::HashSet;

use crate::{
    UPS_TARGET, // pathfinding::PathfindingAgent,
    // units::tasks::{ActionQueue, CurrentAction, CurrentTask},
    UpsCounter,
    items::Inventory,
    map::{
        StructureManager, TILE_SIZE, get_neighbors, is_tile_passable, rounded_tile_pos_to_world,
        world_pos_to_rounded_tile,
    },
};
use bevy::prelude::*;

pub const UNIT_REACH: u8 = 1;
pub const UNIT_DEFAULT_MOVEMENT_SPEED: u32 = UPS_TARGET as u32; // ticks per tile ; smaller is faster (here its 1 tile per second at normal tickrate by default)

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Null,
    NorthWest,
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
}

impl Direction {
    /// Retourne le déplacement (dx, dy) associé à la direction
    pub fn delta(&self) -> IVec2 {
        match self {
            Direction::Null => IVec2::ZERO,
            Direction::NorthWest => IVec2::new(-1, 1),
            Direction::North => IVec2::new(0, 1),
            Direction::NorthEast => IVec2::new(1, 1),
            Direction::East => IVec2::new(1, 0),
            Direction::SouthEast => IVec2::new(1, -1),
            Direction::South => IVec2::new(0, -1),
            Direction::SouthWest => IVec2::new(-1, -1),
            Direction::West => IVec2::new(-1, 0),
        }
    }

    pub fn from(delta: IVec2) -> Self {
        match delta {
            IVec2 { x: 0, y: 0 } => Direction::Null,
            IVec2 { x: -1, y: 1 } => Direction::NorthWest,
            IVec2 { x: 0, y: 1 } => Direction::North,
            IVec2 { x: 1, y: 1 } => Direction::NorthEast,
            IVec2 { x: 1, y: 0 } => Direction::East,
            IVec2 { x: 1, y: -1 } => Direction::SouthEast,
            IVec2 { x: 0, y: -1 } => Direction::South,
            IVec2 { x: -1, y: -1 } => Direction::SouthWest,
            IVec2 { x: -1, y: 0 } => Direction::West,
            _ => Direction::Null, // if direction is wrong
        }
    }
}

#[derive(Component)]
pub struct TileMovement {
    pub direction: Direction,
    pub ticks_per_tile: u32, // movement speed ; smaller is faster
    pub tick_counter: u32,
}

impl Default for TileMovement {
    fn default() -> Self {
        Self {
            direction: Direction::Null,
            ticks_per_tile: UNIT_DEFAULT_MOVEMENT_SPEED,
            tick_counter: 0,
        }
    }
}

impl TileMovement {
    pub fn new(ticks_per_tile: u32) -> Self {
        Self {
            direction: Direction::Null,
            ticks_per_tile,
            tick_counter: 0,
        }
    }

    pub fn update_speed(&mut self, ticks_per_tile: u32) {
        self.ticks_per_tile = ticks_per_tile;
        self.tick_counter = 0;
    }
}

/// to add if the entity needs to checks its collisions with other entities (collisions with walls isn't affected)
#[derive(Component)]
pub struct UnitUnitCollisions;

pub fn move_and_collide_units_system(
    structure_manager: Res<StructureManager>,
    mut unit_query: Query<
        (
            &mut Transform,
            &mut TileMovement,
            Option<&UnitUnitCollisions>,
        ),
        With<Unit>,
    >,
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
        if tile_movement.direction == Direction::Null {
            continue;
        }

        tile_movement.tick_counter += 1;

        if tile_movement.tick_counter >= tile_movement.ticks_per_tile {
            tile_movement.tick_counter = 0;

            let current_tile = world_pos_to_rounded_tile(transform.translation.xy());
            let desired_target_tile = current_tile + tile_movement.direction.delta();
            // if there is a structure
            if !is_tile_passable(desired_target_tile, &structure_manager) {
                tile_movement.direction = Direction::Null;
                continue;
            }

            // if there is a unit with collisions and current unit has collisions too
            if occupied_tiles.contains(&desired_target_tile) && unit_unit_collisions.is_some() {
                tile_movement.direction = Direction::Null;
                continue;
            }

            let target_world_pos = rounded_tile_pos_to_world(desired_target_tile);
            transform.translation.x = target_world_pos.x;
            transform.translation.y = target_world_pos.y;

            tile_movement.direction = Direction::Null;
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
) {
    for (transform, mut tile_movement) in unit_query.iter_mut() {
        // if tile_movement.direction != Direction::Null {
        //     continue;
        // }

        let mut delta = IVec2::new(0, 0);
        if keyboard_input.pressed(KeyCode::ArrowLeft) {
            delta.x -= 1;
        }

        if keyboard_input.pressed(KeyCode::ArrowRight) {
            delta.x += 1;
        }

        if keyboard_input.pressed(KeyCode::ArrowUp) {
            delta.y += 1;
        }

        if keyboard_input.pressed(KeyCode::ArrowDown) {
            delta.y -= 1;
        }

        // if delta == IVec2::ZERO {
        //     continue;
        // }

        let new_direction = Direction::from(delta);
        if tile_movement.direction == new_direction {
            continue;
        }

        tile_movement.direction = new_direction;
        // tile_movement.tick_counter = 0;

        println!("{:?}", tile_movement.direction);
    }
}

// utiliser le tickrate au lieu des secondes pour les déplacements

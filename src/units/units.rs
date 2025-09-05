use std::{collections::HashSet, time::Duration};

use crate::{
    UPS_TARGET, UpsCounter,
    items::Inventory,
    map::{
        StructureManager, TILE_SIZE, get_neighbors, is_tile_passable, rounded_tile_pos_to_world,
        world_pos_to_rounded_tile,
    },
    pathfinding::PathfindingAgent,
    units::tasks::{ActionQueue, CurrentAction, CurrentTask},
};
use bevy::{prelude::*, time::common_conditions::on_timer};
use rand::{Rng, rng};

pub const UNIT_REACH: u8 = 1;
pub const UNIT_DEFAULT_MOVEMENT_SPEED: u32 = UPS_TARGET as u32; // ticks per tile ; smaller is faster (here its 1 tile per second at normal tickrate by default)

pub struct UnitsPlugin;

impl Plugin for UnitsPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_systems(
            FixedUpdate,
            (
                // test_units_control_system.before(move_and_collide_units_system),
                move_and_collide_units_system,
                player_control_system,
                update_sprite_facing_system.after(move_and_collide_units_system),
                display_units_with_no_current_action_system
                    .run_if(on_timer(Duration::from_secs(5))),
                display_units_inventory_system.run_if(on_timer(Duration::from_secs(5))),
            ),
        );
    }
}

#[derive(Component, Debug, Default)]
#[require(
    Sprite,
    Transform,
    TileMovement,
    PathfindingAgent,
    Inventory,
    ActionQueue,
    CurrentAction,
    CurrentTask
)]
pub struct Unit {
    pub name: String,
}

#[derive(Component)]
pub struct Player;

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
    ticks_per_tile: u32, // movement speed ; smaller is faster
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

/// add if the unit should checks its collisions with other units (collisions with walls are not affected by this component)
#[derive(Component)]
pub struct UnitUnitCollisions;

pub fn move_and_collide_units_system(
    structure_manager: Res<StructureManager>,
    mut unit_query: Query<
        (
            Entity,
            &mut Transform,
            &mut TileMovement,
            Has<UnitUnitCollisions>,
        ),
        With<Unit>,
    >,
) {
    // Collecte des tiles occupées en une seule passe
    let occupied_tiles = collect_occupied_tiles(&unit_query);

    // Traitement des mouvements
    for (entity, mut transform, mut tile_movement, has_unit_collisions) in unit_query.iter_mut() {
        if !should_process_movement(&mut tile_movement) {
            continue;
        }

        let current_tile = world_pos_to_rounded_tile(transform.translation.xy());

        match calculate_movement(
            current_tile,
            tile_movement.direction,
            &structure_manager,
            &occupied_tiles,
            has_unit_collisions,
        ) {
            MovementResult::Success {
                target_tile,
                new_direction,
            } => {
                apply_movement(
                    &mut transform,
                    &mut tile_movement,
                    target_tile,
                    new_direction,
                );
            }
            MovementResult::Blocked => {
                tile_movement.direction = Direction::Null;
            }
        }
    }
}

// Structs et enums pour clarifier les intentions
#[derive(Debug)]
enum MovementResult {
    Success {
        target_tile: IVec2,
        new_direction: Direction,
    },
    Blocked,
}

// Fonctions helper pour séparer les responsabilités
fn collect_occupied_tiles(
    unit_query: &Query<
        (
            Entity,
            &mut Transform,
            &mut TileMovement,
            Has<UnitUnitCollisions>,
        ),
        With<Unit>,
    >,
) -> HashSet<IVec2> {
    unit_query
        .iter()
        .filter(|(_, _, _, has_collisions)| *has_collisions)
        .map(|(_, transform, _, _)| world_pos_to_rounded_tile(transform.translation.xy()))
        .collect()
}

fn should_process_movement(tile_movement: &mut TileMovement) -> bool {
    if tile_movement.direction == Direction::Null {
        return false;
    }

    tile_movement.tick_counter += 1;

    if tile_movement.tick_counter >= tile_movement.ticks_per_tile {
        tile_movement.tick_counter = 0;
        true
    } else {
        false
    }
}

fn calculate_movement(
    current_tile: IVec2,
    desired_direction: Direction,
    structure_manager: &Res<StructureManager>,
    occupied_tiles: &HashSet<IVec2>,
    has_unit_collisions: bool,
) -> MovementResult {
    let desired_delta = desired_direction.delta();
    let desired_target = current_tile + desired_delta;

    // Vérification spéciale pour les mouvements diagonaux
    if is_diagonal_movement(desired_delta) {
        if let Some(result) = handle_diagonal_movement(
            current_tile,
            desired_delta,
            structure_manager,
            occupied_tiles,
            has_unit_collisions,
        ) {
            return result;
        }
    }

    // Mouvement direct possible ?
    if can_move_to(
        desired_target,
        structure_manager,
        occupied_tiles,
        has_unit_collisions,
    ) {
        return MovementResult::Success {
            target_tile: desired_target,
            new_direction: desired_direction,
        };
    }

    // Pour les mouvements diagonaux, essayer les axes séparément
    if is_diagonal_movement(desired_delta) {
        try_axis_movement(
            current_tile,
            desired_delta,
            structure_manager,
            occupied_tiles,
            has_unit_collisions,
        )
    } else {
        MovementResult::Blocked
    }
}

fn is_diagonal_movement(delta: IVec2) -> bool {
    delta.x != 0 && delta.y != 0
}

fn handle_diagonal_movement(
    current_tile: IVec2,
    desired_delta: IVec2,
    structure_manager: &Res<StructureManager>,
    occupied_tiles: &HashSet<IVec2>,
    has_unit_collisions: bool,
) -> Option<MovementResult> {
    let tile_x = current_tile + IVec2::new(desired_delta.x, 0);
    let tile_y = current_tile + IVec2::new(0, desired_delta.y);

    // Interdiction si les deux cases orthogonales sont bloquées
    let can_move_x = can_move_to(
        tile_x,
        structure_manager,
        occupied_tiles,
        has_unit_collisions,
    );
    let can_move_y = can_move_to(
        tile_y,
        structure_manager,
        occupied_tiles,
        has_unit_collisions,
    );

    if !can_move_x && !can_move_y {
        Some(MovementResult::Blocked)
    } else {
        None // Continue avec la logique normale
    }
}

fn try_axis_movement(
    current_tile: IVec2,
    desired_delta: IVec2,
    structure_manager: &Res<StructureManager>,
    occupied_tiles: &HashSet<IVec2>,
    has_unit_collisions: bool,
) -> MovementResult {
    let axis_y_tile = current_tile + IVec2::new(0, desired_delta.y);
    let axis_x_tile = current_tile + IVec2::new(desired_delta.x, 0);

    // Priorité Nord/Sud avant Est/Ouest
    if can_move_to(
        axis_y_tile,
        structure_manager,
        occupied_tiles,
        has_unit_collisions,
    ) {
        MovementResult::Success {
            target_tile: axis_y_tile,
            new_direction: Direction::from(IVec2::new(0, desired_delta.y)),
        }
    } else if can_move_to(
        axis_x_tile,
        structure_manager,
        occupied_tiles,
        has_unit_collisions,
    ) {
        MovementResult::Success {
            target_tile: axis_x_tile,
            new_direction: Direction::from(IVec2::new(desired_delta.x, 0)),
        }
    } else {
        MovementResult::Blocked
    }
}

fn apply_movement(
    transform: &mut Transform,
    tile_movement: &mut TileMovement,
    target_tile: IVec2,
    new_direction: Direction,
) {
    let target_world_pos = rounded_tile_pos_to_world(target_tile);
    transform.translation.x = target_world_pos.x;
    transform.translation.y = target_world_pos.y;

    tile_movement.direction = new_direction;
    // Note: Je suppose que tu veux garder la direction, mais ton code original la remet à Null
    // tile_movement.direction = Direction::Null;
}

fn can_move_to(
    tile: IVec2,
    structure_manager: &Res<StructureManager>,
    occupied_tiles: &HashSet<IVec2>,
    has_unit_collisions: bool,
) -> bool {
    is_tile_passable(tile, structure_manager)
        && (!has_unit_collisions || !occupied_tiles.contains(&tile))
}

pub fn update_sprite_facing_system(mut query: Query<(&TileMovement, &mut Transform)>) {
    for (movement, mut transform) in query.iter_mut() {
        if movement.direction != Direction::Null {
            // Détermine la direction horizontale
            let is_moving_left = matches!(
                movement.direction,
                Direction::West | Direction::NorthWest | Direction::SouthWest
            );

            let is_moving_right = matches!(
                movement.direction,
                Direction::East | Direction::NorthEast | Direction::SouthEast
            );

            if is_moving_left {
                transform.scale.x = -transform.scale.x.abs();
            } else if is_moving_right {
                transform.scale.x = transform.scale.x.abs();
            }
        }
    }
}

pub fn player_control_system(
    mut unit_query: Query<&mut TileMovement, (With<Unit>, With<Player>)>,
    input: Res<ButtonInput<KeyCode>>,
) {
    if let Ok(mut tile_movement) = unit_query.single_mut() {
        let mut delta = IVec2::new(0, 0);
        if input.pressed(KeyCode::KeyW) {
            delta.y += 1;
        }
        if input.pressed(KeyCode::KeyA) {
            delta.x -= 1;
        }
        if input.pressed(KeyCode::KeyD) {
            delta.x += 1;
        }
        if input.pressed(KeyCode::KeyS) {
            delta.y -= 1;
        }
        let new_direction = Direction::from(delta);

        if tile_movement.direction != new_direction {
            tile_movement.direction = new_direction;
        }
    }
}

pub fn display_units_with_no_current_action_system(unit_query: Query<&CurrentAction, With<Unit>>) {
    let mut counter = 0;
    for current_action in unit_query.iter() {
        if current_action.action.is_none() {
            counter += 1;
        }
    }
    println!("Counter units with no current action: {}", counter);
}

pub fn display_units_inventory_system(unit_query: Query<&Inventory>) {
    for inventory in unit_query.iter() {
        if !inventory.stackable_items.is_empty() {
            println!("{:?}", inventory);
        }
    }
}

pub fn test_units_control_system(
    mut unit_query: Query<&mut TileMovement, (With<Unit>, Without<Player>)>,
) {
    let mut rng = rand::rng();
    for mut tile_movement in unit_query.iter_mut() {
        let random = rng.random_range(1..=8);

        let new_direction = match random {
            1 => Direction::NorthWest,
            2 => Direction::North,
            3 => Direction::NorthEast,
            4 => Direction::East,
            5 => Direction::SouthEast,
            6 => Direction::South,
            7 => Direction::SouthWest,
            8 => Direction::West,
            _ => Direction::Null,
        };

        if tile_movement.direction != new_direction {
            tile_movement.direction = new_direction;
        }
    }
}

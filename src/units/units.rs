use crate::{
    UpsCounter,
    items::Inventory,
    map::{
        StructureManager, TILE_SIZE, get_neighbors, is_tile_passable, rounded_tile_pos_to_world,
        world_pos_to_rounded_tile,
    },
    pathfinding::PathfindingAgent,
    units::tasks::{CurrentTask, TaskQueue},
};
use bevy::prelude::*;

pub const UNIT_REACH: f32 = 1.0;
pub const UNIT_DEFAULT_MOVEMENT_SPEED: f32 = 5.0;
pub const UNIT_DEFAULT_ROTATION_SPEED: f32 = f32::to_radians(360.0);

#[derive(Component, Debug, Default)]
#[require(
    Sprite,
    Transform,
    MovementSpeed,
    RotationSpeed,
    DesiredMovement,
    PathfindingAgent,
    CircularCollider,
    Inventory,
    TaskQueue,
    CurrentTask
)]
pub struct Unit {
    pub name: String,
}

#[derive(Component)]
pub struct MovementSpeed(pub f32);

impl Default for MovementSpeed {
    fn default() -> Self {
        Self(UNIT_DEFAULT_MOVEMENT_SPEED)
    }
}

#[derive(Component)]
pub struct RotationSpeed(pub f32);

impl Default for RotationSpeed {
    fn default() -> Self {
        Self(UNIT_DEFAULT_ROTATION_SPEED)
    }
}

#[derive(Component, Default)]
pub struct DesiredMovement(pub Vec3);

/// to add if the entity needs to checks its collisions with other entities (collisions with walls isn't affected)
#[derive(Component)]
pub struct UnitUnitCollisions;

#[derive(Component)]
pub struct CircularCollider {
    pub radius: f32,
}

impl Default for CircularCollider {
    fn default() -> Self {
        Self { radius: 0.4 }
    }
}

// Fonction utilitaire pour calculer la collision cercle-rectangle
fn circle_rect_collision(
    circle_center: Vec2,
    circle_radius: f32,
    rect_center: Vec2,
    rect_size: Vec2,
) -> Option<Vec2> {
    // Calculer le point le plus proche du rectangle par rapport au centre du cercle
    let closest_x = circle_center.x.clamp(
        rect_center.x - rect_size.x / 2.0,
        rect_center.x + rect_size.x / 2.0,
    );
    let closest_y = circle_center.y.clamp(
        rect_center.y - rect_size.y / 2.0,
        rect_center.y + rect_size.y / 2.0,
    );

    let closest_point = Vec2::new(closest_x, closest_y);
    let distance_vec = circle_center - closest_point;
    let distance = distance_vec.length();

    if distance < circle_radius {
        // Il y a collision, retourner le vecteur de résolution
        if distance > 0.0 {
            let penetration_depth = circle_radius - distance;
            Some(distance_vec.normalize() * penetration_depth)
        } else {
            // Le centre du cercle est exactement sur le point le plus proche
            // Choisir une direction par défaut (vers le haut)
            Some(Vec2::Y * circle_radius)
        }
    } else {
        None
    }
}

pub fn update_logic(
    mut counter: ResMut<UpsCounter>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut unit_query: Query<
        (
            &MovementSpeed,
            &RotationSpeed,
            &mut Transform,
            &mut DesiredMovement,
        ),
        With<Unit>,
    >,
    time: Res<Time>,
) {
    counter.ticks += 1;

    for (movement_speed, rotation_speed, mut transform, mut desired_movement) in
        unit_query.iter_mut()
    {
        let mut rotation_factor = 0.0;
        let mut movement_factor = 0.0;

        if keyboard_input.pressed(KeyCode::ArrowLeft) {
            rotation_factor += 1.0;
        }

        if keyboard_input.pressed(KeyCode::ArrowRight) {
            rotation_factor -= 1.0;
        }

        if keyboard_input.pressed(KeyCode::ArrowUp) {
            movement_factor += 1.0;
        }

        // Update the ship rotation around the Z axis (perpendicular to the 2D plane of the screen)
        transform.rotate_z(rotation_factor * rotation_speed.0 * time.delta_secs());

        let movement_direction = transform.rotation * Vec3::Y;
        let movement_distance_tiles = movement_factor * movement_speed.0 * time.delta_secs();
        let movement_distance_pixels = movement_distance_tiles * TILE_SIZE.x;
        let translation_delta = movement_direction * movement_distance_pixels;

        desired_movement.0 = translation_delta;
    }
}

/// Déplace les unités et gère les collisions.
pub fn move_and_collide_units(
    structure_manager: Res<StructureManager>,
    mut unit_query: Query<(&mut Transform, &DesiredMovement, &CircularCollider), With<Unit>>,
) {
    for (mut transform, desired_movement, collider) in unit_query.iter_mut() {
        let proposed_position = transform.translation + desired_movement.0;
        let mut final_position = proposed_position;

        // Convertir en position tuile et obtenir les tuiles voisines
        let current_tile_pos = world_pos_to_rounded_tile(transform.translation.xy());
        let neighbor_tiles = get_neighbors(current_tile_pos);

        // Vérifier les collisions avec les structures voisines
        for tile_pos in neighbor_tiles {
            if !is_tile_passable(tile_pos, &structure_manager) {
                let tile_world_center = rounded_tile_pos_to_world(tile_pos);
                let collider_radius_pixels = collider.radius * TILE_SIZE.x;

                if let Some(resolution) = circle_rect_collision(
                    proposed_position.xy(),
                    collider_radius_pixels,
                    tile_world_center,
                    TILE_SIZE.into(),
                ) {
                    // Ajuster la position en cas de collision
                    final_position.x += resolution.x;
                    final_position.y += resolution.y;
                }
            }
        }

        transform.translation = final_position;
    }
}

/// handles collisions between units with ActiveCollisions component
pub fn unit_unit_collisions(
    mut unit_query: Query<
        (&mut Transform, &CircularCollider),
        (With<Unit>, With<UnitUnitCollisions>),
    >,
) {
    let mut combinations = unit_query.iter_combinations_mut();
    while let Some([mut unit_a, mut unit_b]) = combinations.fetch_next() {
        // Déstructure une seule fois en bindings mutables
        let (transform_a, collider_a) = &mut unit_a;
        let (transform_b, collider_b) = &mut unit_b;

        // Snapshot positions en 2D (avant modification)
        let pos_a = transform_a.translation.xy();
        let pos_b = transform_b.translation.xy();

        // Rayons en pixels
        let radius_a_pixels = collider_a.radius * TILE_SIZE.x;
        let radius_b_pixels = collider_b.radius * TILE_SIZE.x;
        let min_distance = radius_a_pixels + radius_b_pixels;

        let distance = pos_a.distance(pos_b);

        if distance < min_distance {
            let overlap = min_distance - distance;
            // direction 2D, en évitant la division par zéro
            let dir2 = (pos_a - pos_b).normalize_or_zero();
            let dir3 = dir2.extend(0.0); // convertir en Vec3 pour appliquer sur Transform

            // Appliquer la résolution équitablement
            transform_a.translation += dir3 * (overlap / 2.0);
            transform_b.translation -= dir3 * (overlap / 2.0);
        }
    }
}

pub fn display_units_with_no_current_task(unit_query: Query<&CurrentTask, With<Unit>>) {
    let mut counter = 0;
    for current_task in unit_query.iter() {
        if current_task.task.is_none() {
            counter += 1;
        }
    }
    println!("Counter units with no current task: {}", counter);
}

pub fn display_units_inventory(unit_query: Query<&Inventory>) {
    for inventory in unit_query.iter() {
        println!("{:?}", inventory);
    }
}

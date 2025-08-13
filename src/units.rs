use crate::{
    UpsCounter,
    map::{TILE_SIZE, Wall},
};
use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;

#[derive(Component)]
pub struct Unit {
    pub movement_speed: f32,
    pub rotation_speed: f32,
}

#[derive(Component, Default)]
pub struct DesiredMovement(Vec3);

#[derive(Component)]
pub struct CircularCollider {
    pub radius: f32,
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
    query: Query<(&Unit, &mut Transform, &mut DesiredMovement)>,
    time: Res<Time>,
) {
    counter.ticks += 1;

    for entity in query {
        let (ship, mut transform, mut desired_movement) = entity;

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
        transform.rotate_z(rotation_factor * ship.rotation_speed * time.delta_secs());

        // Get the ship's forward vector by applying the current rotation to the ships initial facing
        // vector
        let movement_direction = transform.rotation * Vec3::Y;
        // Get the distance the ship will move based on direction, the ship's movement speed and delta
        // time
        let movement_distance_tiles = movement_factor * ship.movement_speed * time.delta_secs();
        let movement_distance_pixels = movement_distance_tiles * TILE_SIZE.x;
        // Create the change in translation using the new movement direction and distance
        let translation_delta = movement_direction * movement_distance_pixels;
        // Update the ship translation with our new translation delta
        desired_movement.0 = translation_delta;
    }
}

/// Déplace les unités et gère les collisions.
pub fn move_and_collide_units(
    mut unit_query: Query<(&mut Transform, &DesiredMovement, &CircularCollider), With<Unit>>,
    wall_query: Query<(&TilePos, &TilemapId), (With<Wall>, Without<Unit>)>,
    tilemap_q: Query<(&TilemapGridSize, &Transform), (With<TileStorage>, Without<Unit>)>,
) {
    // 1) MOUVEMENT DES UNITÉS
    for (mut transform, desired_movement, collider) in unit_query.iter_mut() {
        // 2) GESTION DES COLLISIONS UNIT-MUR
        let proposed_position = transform.translation + desired_movement.0;
        let mut final_position = proposed_position;

        // Vérifier les collisions avec tous les murs
        for (wall_tile_pos, wall_tilemap_id) in wall_query.iter() {
            // Trouver la tilemap correspondante
            if let Ok((grid_size, tilemap_transform)) = tilemap_q.get(wall_tilemap_id.0) {
                // Calculer la position monde du mur
                let tile_world_pos = Vec2::new(
                    tilemap_transform.translation.x + wall_tile_pos.x as f32 * grid_size.x,
                    tilemap_transform.translation.y + wall_tile_pos.y as f32 * grid_size.y,
                );

                let tile_size = Vec2::new(grid_size.x, grid_size.y);
                // Convertir le rayon de collision de tiles en pixels
                let collider_radius_pixels = collider.radius * TILE_SIZE.x;

                // Vérifier la collision cercle-rectangle
                if let Some(resolution_vector) = circle_rect_collision(
                    Vec2::new(proposed_position.x, proposed_position.y),
                    collider_radius_pixels,
                    tile_world_pos,
                    tile_size,
                ) {
                    // Il y a collision, ajuster la position
                    final_position.x += resolution_vector.x;
                    final_position.y += resolution_vector.y;
                }
            }
        }

        //TODO: modifier pour éviter de faire la logique en pixels, et faire la convertions en pixels à la fin

        // Appliquer la position finale
        transform.translation = final_position;
    }

    // 3) GESTION DES COLLISIONS UNIT-UNIT
    let mut combinations = unit_query.iter_combinations_mut();
    while let Some([mut unit_a, mut unit_b]) = combinations.fetch_next() {
        // Déstructure une seule fois en bindings mutables
        let (transform_a, _desired_a, collider_a) = &mut unit_a;
        let (transform_b, _desired_b, collider_b) = &mut unit_b;

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

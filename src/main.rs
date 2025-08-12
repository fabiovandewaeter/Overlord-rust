use bevy::{
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    input::mouse::{MouseScrollUnit, MouseWheel},
    prelude::*,
};
use bevy_ecs_tilemap::{map::*, tiles::*};
use rand::Rng;

use crate::map::{ChunkManager, MapPlugin, TILE_SIZE, Wall, camera_pos_to_chunk_pos, spawn_chunk};

mod map;

const TARGET_UPS: f64 = 30.0;
const ZOOM_IN_SPEED: f32 = 0.25 / 400000000.0;
const ZOOM_OUT_SPEED: f32 = 4.0 * 400000000.0;
const CAMERA_SPEED: f32 = 1200.0;

#[derive(Resource)]
struct UpsCounter {
    ticks: u32,
    last_second: f64,
    ups: u32,
}

#[derive(Component)]
struct Unit {
    movement_speed: f32,
    rotation_speed: f32,
}

#[derive(Component)]
struct CircularCollider {
    pub radius: f32,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    asset_server: Res<AssetServer>,
) {
    use bevy::color::palettes::css::GREEN;

    let player_texture_handle = asset_server.load("default.png");
    let mut rng = rand::rng();
    for i in 0..10 {
        let random_number: i32 = rng.random_range(0..500); // un entier de 0 à 9

        commands.spawn((
            Sprite::from_image(player_texture_handle.clone()),
            Unit {
                // movement_speed: 500.0,
                movement_speed: random_number as f32,
                rotation_speed: f32::to_radians(360.0),
            },
            CircularCollider {
                radius: TILE_SIZE.x * 0.4,
            },
        ));
    }

    commands.spawn((Camera2d, Camera { ..default() }));
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::new(40.0, 20.0))),
        MeshMaterial2d(materials.add(Color::from(GREEN))),
    ));
}

fn handle_camera_inputs(
    mut camera_query: Query<(&mut Camera, &mut Transform, &mut Projection)>,
    input: Res<ButtonInput<KeyCode>>,
    mut input_mouse_wheel: EventReader<MouseWheel>,
    time: Res<Time>,
) {
    let Ok((mut _camera, mut transform, mut projection)) = camera_query.single_mut() else {
        return;
    };

    // Camera movement controls
    let mut direction = Vec3::ZERO;

    if input.pressed(KeyCode::KeyW) {
        direction.y += 1.0;
    }
    if input.pressed(KeyCode::KeyS) {
        direction.y -= 1.0;
    }
    if input.pressed(KeyCode::KeyA) {
        direction.x -= 1.0;
    }
    if input.pressed(KeyCode::KeyD) {
        direction.x += 1.0;
    }

    // Récupérer le niveau de zoom actuel
    let zoom_scale = if let Projection::Orthographic(projection2d) = &*projection {
        projection2d.scale
    } else {
        1.0 // Valeur par défaut si ce n'est pas une projection orthographique
    };

    // normalizes to have constant diagonal speed
    if direction != Vec3::ZERO {
        direction = direction.normalize();
        let speed = CAMERA_SPEED * zoom_scale.powf(0.7) * time.delta_secs();
        transform.translation += direction * speed;
    }

    // Camera zoom controls
    if let Projection::Orthographic(projection2d) = &mut *projection {
        for mouse_wheel_event in input_mouse_wheel.read() {
            use bevy::math::ops::powf;
            match mouse_wheel_event.unit {
                MouseScrollUnit::Line => {
                    if mouse_wheel_event.y > 0.0 {
                        projection2d.scale *= powf(ZOOM_IN_SPEED, time.delta_secs());
                    } else if mouse_wheel_event.y < 0.0 {
                        projection2d.scale *= powf(ZOOM_OUT_SPEED, time.delta_secs());
                    }
                }
                MouseScrollUnit::Pixel => {
                    if mouse_wheel_event.y > 0.0 {
                        projection2d.scale *= powf(ZOOM_IN_SPEED, time.delta_secs());
                    } else if mouse_wheel_event.y < 0.0 {
                        projection2d.scale *= powf(ZOOM_OUT_SPEED, time.delta_secs());
                    }
                }
            }
        }
    }
}

fn update_logic(
    mut counter: ResMut<UpsCounter>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    // query: Single<(&Entity, &mut Transform)>,
    query: Query<(&Unit, &mut Transform)>,
    time: Res<Time>,
) {
    counter.ticks += 1;

    for entity in query {
        // let (ship, mut transform) = query.into_inner();
        let (ship, mut transform) = entity;

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
        let movement_distance = movement_factor * ship.movement_speed * time.delta_secs();
        // Create the change in translation using the new movement direction and distance
        let translation_delta = movement_direction * movement_distance;
        // Update the ship translation with our new translation delta
        transform.translation += translation_delta;
    }
}

fn display_fps_ups(
    time: Res<Time>,
    diagnostics: Res<DiagnosticsStore>,
    mut counter: ResMut<UpsCounter>,
) {
    let now = time.elapsed_secs_f64();
    if now - counter.last_second >= 1.0 {
        // Calcule l’UPS
        counter.ups = counter.ticks;
        counter.ticks = 0;
        counter.last_second = now;

        // Récupère le FPS depuis le plugin
        if let Some(fps) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS) {
            if let Some(fps_avg) = fps.smoothed() {
                println!("FPS: {:.0} | UPS: {}", fps_avg, counter.ups);
            }
        }
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

/// Déplace les unités et gère les collisions.
fn move_and_collide_units(
    mut unit_query: Query<(Entity, &Unit, &mut Transform, &CircularCollider)>,
    wall_query: Query<(&TilePos, &TilemapId), (With<Wall>, Without<Unit>)>,
    tilemap_q: Query<(&TilemapGridSize, &Transform), (With<TileStorage>, Without<Unit>)>,
    time: Res<Time>,
) {
    let delta_time = time.delta_secs();

    // 1) MOUVEMENT DES UNITÉS
    for (_entity, unit, mut transform, _collider) in unit_query.iter_mut() {
        // Mouvement simple : les unités tournent et avancent
        transform.rotate_z(unit.rotation_speed * delta_time * 0.1);
        let movement_direction = transform.up();
        let movement_amount = unit.movement_speed * delta_time;

        // Position proposée après mouvement
        let proposed_position = transform.translation + movement_direction * movement_amount;

        // 2) GESTION DES COLLISIONS UNIT-MUR
        let mut final_position = proposed_position;
        let unit_pos_2d = Vec2::new(proposed_position.x, proposed_position.y);

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

                // Vérifier la collision cercle-rectangle
                if let Some(resolution_vector) =
                    circle_rect_collision(unit_pos_2d, _collider.radius, tile_world_pos, tile_size)
                {
                    // Il y a collision, ajuster la position
                    final_position.x += resolution_vector.x;
                    final_position.y += resolution_vector.y;
                }
            }
        }

        // Appliquer la position finale
        transform.translation = final_position;
    }

    // 3) GESTION DES COLLISIONS UNIT-UNIT
    let mut combinations = unit_query.iter_combinations_mut();
    while let Some([unit_a, unit_b]) = combinations.fetch_next() {
        let (_entity_a, _unit_a, transform_a, collider_a) = &unit_a;
        let (_entity_b, _unit_b, transform_b, collider_b) = &unit_b;

        let distance = transform_a.translation.distance(transform_b.translation);
        let min_distance = collider_a.radius + collider_b.radius;

        if distance < min_distance {
            let overlap = min_distance - distance;
            let direction = (transform_a.translation - transform_b.translation).normalize_or_zero();

            let (_, _, mut transform_a_mut, _) = unit_a;
            transform_a_mut.translation += direction * overlap / 2.0;

            let (_, _, mut transform_b_mut, _) = unit_b;
            transform_b_mut.translation -= direction * overlap / 2.0;
        }
    }
}

fn spawn_chunks_around_units(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    units_query: Query<&Transform, With<Unit>>,
    mut chunk_manager: ResMut<ChunkManager>,
) {
    // for transform in camera_query.iter() {
    for unit_transform in units_query {
        let camera_chunk_pos = camera_pos_to_chunk_pos(&unit_transform.translation.xy());
        for y in (camera_chunk_pos.y - 2)..(camera_chunk_pos.y + 2) {
            for x in (camera_chunk_pos.x - 2)..(camera_chunk_pos.x + 2) {
                if !chunk_manager.spawned_chunks.contains(&IVec2::new(x, y)) {
                    chunk_manager.spawned_chunks.insert(IVec2::new(x, y));
                    spawn_chunk(&mut commands, &asset_server, IVec2::new(x, y));
                }
            }
        }
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Overlord".to_string(),
                present_mode: bevy::window::PresentMode::AutoVsync,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
        .add_plugins(MapPlugin)
        .insert_resource(UpsCounter {
            ticks: 0,
            last_second: 0.0,
            ups: 0,
        })
        .insert_resource(Time::<Fixed>::from_hz(TARGET_UPS))
        .add_systems(Startup, setup)
        .add_systems(Update, (handle_camera_inputs, display_fps_ups))
        .add_systems(
            FixedUpdate,
            (
                update_logic,
                move_and_collide_units,
                spawn_chunks_around_units,
            ),
        )
        .run();
}

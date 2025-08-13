use std::collections::VecDeque;

use bevy::{
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    input::mouse::{MouseScrollUnit, MouseWheel},
    prelude::*,
};
use bevy_ecs_tilemap::{map::*, tiles::*};

use crate::{
    map::{
        ChunkManager, MapPlugin, TILE_SIZE, Wall, camera_pos_to_chunk_pos, spawn_chunk,
        tile_coords_to_world,
    },
    pathfinding::{PathfindingAgent, PathfindingPlugin},
    units::{CircularCollider, DesiredMovement, Unit, move_and_collide_units, update_logic},
};

mod map;
mod pathfinding;
mod units;

const TARGET_UPS: f64 = 30.0;
const ZOOM_IN_SPEED: f32 = 0.25 / 400000000.0;
const ZOOM_OUT_SPEED: f32 = 4.0 * 400000000.0;
const CAMERA_SPEED: f32 = 37.5;

#[derive(Resource)]
struct UpsCounter {
    ticks: u32,
    last_second: f64,
    ups: u32,
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

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    asset_server: Res<AssetServer>,
) {
    use bevy::color::palettes::css::GREEN;

    let player_texture_handle = asset_server.load("default.png");
    let mut rng = rand::rng();
    for _i in 0..1 {
        // let random_number: i32 = rng.random_range(0..5); // un entier de 0 à 9
        let random_number: i32 = 5;

        let world_pos = tile_coords_to_world(Vec2::new(0.5, 0.5));

        commands.spawn((
            Sprite::from_image(player_texture_handle.clone()),
            Transform::from_translation(world_pos.extend(1.0)),
            DesiredMovement::default(),
            Unit {
                movement_speed: random_number as f32,
                rotation_speed: f32::to_radians(360.0),
            },
            PathfindingAgent {
                target: None,
                path: VecDeque::new(),
                speed: random_number as f32,
                path_tolerance: 0.1, // 10% de la taille d'une tile
            },
            CircularCollider { radius: 0.4 },
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
        let speed_in_pixels = CAMERA_SPEED * TILE_SIZE.x * zoom_scale.powf(0.7) * time.delta_secs();
        transform.translation += direction * speed_in_pixels;
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
        .add_plugins(PathfindingPlugin)
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
                // spawn_chunks_around_units,
            ),
        )
        .run();
}

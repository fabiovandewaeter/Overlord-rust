use crate::{
    items::{Inventory, ItemKind, display_inventories},
    map::{Chest, MapPlugin, SolidStructure, TILE_SIZE, tile_pos_to_world},
    pathfinding::{PathfindingAgent, PathfindingPlugin},
    units::{
        CircularCollider, DesiredMovement, Unit, display_units_with_no_current_task,
        move_and_collide_units,
        states::Available,
        tasks::{CurrentTask, TaskQueue, TasksPlugin},
        unit_unit_collisions, update_logic,
    },
};
use bevy::{
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    input::{
        common_conditions::input_pressed,
        mouse::{MouseScrollUnit, MouseWheel},
    },
    prelude::*,
    time::common_conditions::on_timer,
};
use std::{collections::VecDeque, time::Duration};

mod items;
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
    for _i in 0..1 {
        let random_number: i32 = 5;

        let world_pos = tile_pos_to_world(Vec2::new(0.5, 0.5));

        // unit
        commands.spawn((
            Sprite::from_image(player_texture_handle.clone()),
            Transform::from_translation(world_pos.extend(1.0)),
            DesiredMovement::default(),
            Unit {
                movement_speed: random_number as f32,
                rotation_speed: f32::to_radians(360.0),
            },
            // keep that component because entities will always move so it's useless to add/remove it everytime
            PathfindingAgent {
                target: None,
                path: VecDeque::new(),
                speed: random_number as f32,
                path_tolerance: 0.1, // 10% de la taille d'une tile
            },
            Available,
            CircularCollider { radius: 0.4 },
            // ActiveCollisions,
            Inventory::new(),
            TaskQueue::from(vec![]),
            CurrentTask(None),
        ));
    }

    // chest
    let world_pos = tile_pos_to_world(Vec2::new(5.5, 0.5));
    let mut inventory = Inventory::new();
    inventory.add(ItemKind::Rock, 1000);
    commands.spawn((
        Transform::from_translation(world_pos.extend(1.0)),
        SolidStructure,
        // Inventory::new(),
        inventory,
        Chest,
    ));

    commands.spawn((Camera2d, Camera { ..default() }));
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::new(20.0, 20.0))),
        MeshMaterial2d(materials.add(Color::from(GREEN))),
    ));
}

fn handle_camera_inputs(
    mut camera_query: Query<(&mut Transform, &mut Projection), With<Camera>>,
    input: Res<ButtonInput<KeyCode>>,
    mut input_mouse_wheel: EventReader<MouseWheel>,
    time: Res<Time>,
) {
    let Ok((mut transform, mut projection)) = camera_query.single_mut() else {
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
        .add_plugins(TasksPlugin)
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
                unit_unit_collisions.after(move_and_collide_units),
                display_inventories.run_if(input_pressed(KeyCode::KeyI)),
                display_units_with_no_current_task.run_if(on_timer(Duration::from_secs(1))),
            ),
        )
        .run();
}

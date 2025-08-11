use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;

const TARGET_UPS: f64 = 30.0;
const ZOOM_IN_SPEED: f32 = 0.25 / 400000000.0;
const ZOOM_OUT_SPEED: f32 = 4.0 * 400000000.0;

#[derive(Resource)]
struct UpsCounter {
    ticks: u32,
    last_second: f64,
    ups: u32,
}

#[derive(Component)]
struct Entity {
    movement_speed: f32,
    rotation_speed: f32,
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
        .add_systems(Startup, setup)
        .insert_resource(UpsCounter {
            ticks: 0,
            last_second: 0.0,
            ups: 0,
        })
        .insert_resource(Time::<Fixed>::from_hz(TARGET_UPS))
        .add_systems(Update, (handle_camera_inputs, display_fps_ups))
        .add_systems(FixedUpdate, update_logic)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    asset_server: Res<AssetServer>,
) {
    use bevy::color::palettes::css::GREEN;

    let player_texture_handle = asset_server.load("default.png");
    commands.spawn((
        Sprite::from_image(player_texture_handle),
        Entity {
            movement_speed: 500.0,
            rotation_speed: f32::to_radians(360.0),
        },
    ));

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
    // normalizes to have constant diagonal speed
    if direction != Vec3::ZERO {
        direction = direction.normalize();
        let speed = 600.0 * time.delta_secs();
        transform.translation += direction * speed;
    }

    // Camera zoom controls
    if let Projection::Orthographic(projection2d) = &mut *projection {
        for mouse_wheel_event in input_mouse_wheel.read() {
            use bevy::math::ops::powf;
            match mouse_wheel_event.unit {
                MouseScrollUnit::Line => {
                    println!(
                        "Scroll (line units): vertical: {}, horizontal: {}",
                        mouse_wheel_event.y, mouse_wheel_event.x
                    );
                    if mouse_wheel_event.y > 0.0 {
                        projection2d.scale *= powf(ZOOM_IN_SPEED, time.delta_secs());
                    } else if mouse_wheel_event.y < 0.0 {
                        projection2d.scale *= powf(ZOOM_OUT_SPEED, time.delta_secs());
                    }
                }
                MouseScrollUnit::Pixel => {
                    println!(
                        "Scroll (pixel units): vertical: {}, horizontal: {}",
                        mouse_wheel_event.y, mouse_wheel_event.x
                    );
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

const BOUNDS: Vec2 = Vec2::new(1200.0, 640.0);

fn update_logic(
    mut counter: ResMut<UpsCounter>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    query: Single<(&Entity, &mut Transform)>,
    time: Res<Time>,
) {
    counter.ticks += 1;

    let (ship, mut transform) = query.into_inner();

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

    // Bound the ship within the invisible level bounds
    let extents = Vec3::from((BOUNDS / 2.0, 0.0));
    transform.translation = transform.translation.min(extents).max(-extents);
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

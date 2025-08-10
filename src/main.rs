use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::time::common_conditions::on_timer;
use std::time::Duration;

#[derive(Resource)]
struct UpsCounter {
    ticks: u32,
    last_second: f64,
    ups: u32,
}

fn main() {
    let target_ups: f32 = 30.0;

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Overlord".to_string(),
                present_mode: bevy::window::PresentMode::AutoVsync, // pas de vsync -> FPS max
                ..default()
            }),
            ..default()
        }))
        // Plugin pour mesurer FPS
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
        // UPS tracker
        .insert_resource(UpsCounter {
            ticks: 0,
            last_second: 0.0,
            ups: 0,
        })
        // Logique à 30 UPS
        .add_systems(
            Update,
            update_logic.run_if(on_timer(Duration::from_secs_f32(1.0 / target_ups))),
        )
        // Rendu à chaque frame
        .add_systems(Update, update_render)
        // Affichage FPS / UPS toutes les secondes
        .add_systems(Update, display_fps_ups)
        .run();
}

fn update_logic(mut counter: ResMut<UpsCounter>, time: Res<Time>) {
    counter.ticks += 1;

    // Exemple de logique : déplacer un truc, calculer une simulation...
}

fn update_render() {
    // Rendu visuel à chaque frame
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

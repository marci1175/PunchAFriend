mod systems;
mod ui;

use bevy::{log::LogPlugin, prelude::*};
use bevy_egui::EguiPlugin;
use bevy_rapier2d::{
    plugin::{NoUserData, RapierPhysicsPlugin},
    render::RapierDebugRenderPlugin,
};
use punchafriend::{
    game::{
        collision::{check_for_collision_with_attack_object, CollisionGroupSet},
        RandomEngine,
    },
    server::ApplicationCtx,
};

fn main() {
    // console_subscriber::init();

    let mut app = App::new();

    app.add_plugins(DefaultPlugins.build().add(LogPlugin {
        filter: "info,wgpu_core=warn,wgpu_hal=off,tokio=trace,runtime=trace".into(),
        level: bevy::log::Level::DEBUG,
        ..Default::default()
    }));
    app.add_plugins(EguiPlugin);
    app.add_plugins(bevy_framepace::FramepacePlugin);
    app.add_plugins(bevy_tokio_tasks::TokioTasksPlugin::default());
    app.add_plugins(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(100.0));
    app.add_plugins(RapierDebugRenderPlugin::default());

    app.insert_resource(ApplicationCtx::default());
    app.insert_resource(CollisionGroupSet::new());
    app.insert_resource(RandomEngine::new());

    app.add_systems(Startup, systems::setup_window);
    app.add_systems(Update, systems::tick);
    app.add_systems(Update, systems::reset_jump_remaining_for_player);
    app.add_systems(Update, check_for_collision_with_attack_object);
    app.add_systems(Update, ui::ui_system);

    app.run();
}

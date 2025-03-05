mod ui;

use bevy::{
    app::{App, PluginGroup, Update},
    log::LogPlugin,
    DefaultPlugins,
};
use bevy_egui::EguiPlugin;
use bevy_rapier2d::{plugin::{NoUserData, RapierPhysicsPlugin}, render::RapierDebugRenderPlugin};
use punchafriend::{client::ApplicationCtx, game::collision::CollisionGroupSet};
use ui::ui_system;

fn main() {
    let mut app = App::new();

    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    app.add_plugins(DefaultPlugins.build().add(LogPlugin {
        filter: "info,wgpu_core=warn,wgpu_hal=off".into(),
        level: bevy::log::Level::DEBUG,
        ..Default::default()
    }));

    app.add_plugins(EguiPlugin);
    app.add_plugins(bevy_tokio_tasks::TokioTasksPlugin::default());
    app.add_plugins(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(100.0));
    app.add_plugins(RapierDebugRenderPlugin::default());
    
    app.insert_resource(ApplicationCtx::default());
    app.insert_resource(CollisionGroupSet::default());

    app.add_systems(Update, ui_system);

    app.run();
}

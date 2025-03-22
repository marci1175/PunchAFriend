mod app;
mod systems;
mod ui;

use bevy::{
    app::{App, FixedUpdate, PluginGroup, Startup, Update},
    log::LogPlugin,
    render::texture::ImagePlugin,
    DefaultPlugins,
};
use bevy_egui::EguiPlugin;
use bevy_rapier2d::{
    plugin::{NoUserData, RapierPhysicsPlugin},
    render::RapierDebugRenderPlugin,
};
use punchafriend::{client::ApplicationCtx, game::collision::CollisionGroupSet};
use systems::{exit_handler, handle_server_output, handle_user_input, setup_game};
use ui::ui_system;

fn main() {
    let mut app = App::new();

    app.add_plugins(
        DefaultPlugins
            .build()
            .set(LogPlugin {
                filter: "info,wgpu_core=warn,wgpu_hal=off".into(),
                level: bevy::log::Level::DEBUG,
                ..Default::default()
            })
            .set(ImagePlugin::default_nearest()),
    );

    app.add_plugins(EguiPlugin);
    app.add_plugins(bevy_framepace::FramepacePlugin);
    app.add_plugins(bevy_tokio_tasks::TokioTasksPlugin::default());
    app.add_plugins(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(100.0));
    app.add_plugins(RapierDebugRenderPlugin::default());

    app.insert_resource(ApplicationCtx::default());
    app.insert_resource(CollisionGroupSet::default());

    app.add_systems(Startup, setup_game);
    app.add_systems(Update, ui_system);
    app.add_systems(FixedUpdate, handle_server_output);
    app.add_systems(Update, handle_user_input);
    app.add_systems(Update, exit_handler);

    app.run();
}

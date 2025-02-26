mod systems;

use bevy::prelude::*;
use bevy_egui::EguiPlugin;
use bevy_rapier2d::{
    plugin::{NoUserData, RapierPhysicsPlugin},
    render::RapierDebugRenderPlugin,
};
use punchafriend::{ApplicationCtx, ClientConnection, CollisionGroupSet};

#[tokio::main]
async fn main() {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins);
    app.add_plugins(EguiPlugin);
    app.add_plugins(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(100.0));
    app.add_plugins(RapierDebugRenderPlugin::default());

    app.insert_resource(ClientConnection::default());
    app.insert_resource(ApplicationCtx::default());
    app.insert_resource(CollisionGroupSet::new());

    app.add_systems(Update, systems::frame);
    app.add_systems(Update, systems::reset_jump_remaining_for_self_chrac);
    app.add_systems(Update, systems::check_for_collision_with_attack_object);
    app.add_systems(Update, systems::ui_system);

    app.run();
}

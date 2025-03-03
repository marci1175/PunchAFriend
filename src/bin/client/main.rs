mod ui;

use bevy::{
    app::{App, Update},
    DefaultPlugins,
};
use bevy_egui::EguiPlugin;
use ui::{ui_system, ApplicationCtx};

fn main() {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins);
    app.add_plugins(EguiPlugin);
    app.add_plugins(bevy_tokio_tasks::TokioTasksPlugin::default());

    app.insert_resource(ApplicationCtx::default());

    app.add_systems(Update, ui_system);

    app.run();
}

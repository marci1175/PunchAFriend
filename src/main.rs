mod systems;

use bevy::{
    app::{App, Startup, Update},
    DefaultPlugins,
};

fn main() {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins);

    app.add_systems(Startup, systems::setup);
    app.add_systems(Update, systems::frame);

    app.run();
}

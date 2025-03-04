pub mod game;
pub mod networking;

use bevy::{ecs::component::Component, math::Vec2};

#[derive(Component, Clone)]
/// A MapElement instnace is an object which is a part of the map.
/// This is used to make difference between Entities which are a part of the obstacles contained in the map.
pub struct MapElement;

#[derive(Component, Clone)]
pub struct MapObject {
    pub size: Vec2,
    pub avoid_collision_from: Direction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Direction {
    Left,
    #[default]
    Right,
    Up,
    Down,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum UiMode {
    Game,
    #[default]
    MainMenu,
    GameMenu,
    PauseWindow,
}

pub mod server {
    use std::net::SocketAddr;

    use bevy::ecs::system::Resource;
    use quinn::rustls::pki_types::CertificateDer;
    use rand::{rngs::SmallRng, SeedableRng};
    use tokio::sync::mpsc::{channel, Receiver};

    use crate::{networking::server::ServerInstance, UiMode};

    #[derive(Resource)]
    pub struct ApplicationCtx {
        /// The Ui's state in the Application.
        pub ui_state: UiMode,

        /// Startup initalized [`SmallRng`] random generator.
        /// Please note, that the [`SmallRng`] is insecure and should not be used in crypto contexts.
        pub rand: rand::rngs::SmallRng,

        pub server_instance_receiver:
            Receiver<anyhow::Result<(ServerInstance, CertificateDer<'static>, SocketAddr)>>,

        pub server_instance: Option<ServerInstance>,
    }

    impl Default for ApplicationCtx {
        fn default() -> Self {
            Self {
                ui_state: UiMode::MainMenu,
                rand: SmallRng::from_rng(&mut rand::rng()),
                server_instance_receiver: channel(255).1,
                server_instance: None,
            }
        }
    }
}

mod client {

    use bevy::ecs::system::Resource;

    use egui_toast::Toasts;

    use rand::{rngs::SmallRng, SeedableRng};
    use tokio::sync::mpsc::{channel, Receiver};

    use crate::{networking::client::ClientConnection, UiMode};

    #[derive(Resource)]
    pub struct ApplicationCtx {
        /// The Ui's mode in the Application.
        pub ui_mode: UiMode,

        /// The Ui's state in the Application,
        pub ui_state: UiMode,

        /// Startup initalized [`SmallRng`] random generator.
        /// Please note, that the [`SmallRng`] is insecure and should not be used in crypto contexts.
        pub rand: rand::rngs::SmallRng,

        /// The Client's currently ongoing connection to a remote address.
        pub client_connection: Option<ClientConnection>,

        /// Receives the connecting threads connection result.
        pub connection_receiver: Receiver<anyhow::Result<ClientConnection>>,

        /// Used to display notifications with egui
        pub egui_toasts: Toasts,
    }

    impl Default for ApplicationCtx {
        fn default() -> Self {
            Self {
                ui_mode: UiMode::MainMenu,
                ui_state: UiMode::default(),
                client_connection: None,
                rand: SmallRng::from_rng(&mut rand::rng()),
                connection_receiver: channel(255).1,
                egui_toasts: Toasts::new(),
            }
        }
    }
}

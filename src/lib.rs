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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Deserialize, serde::Serialize)]
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
    use std::sync::Arc;

    use bevy::ecs::system::Resource;
    use parking_lot::RwLock;

    use rand::{rngs::SmallRng, SeedableRng};
    use tokio::sync::mpsc::{channel, Receiver};
    use tokio_util::sync::CancellationToken;

    use crate::{
        networking::server::{RemoteGameClient, ServerInstance},
        UiMode,
    };

    #[derive(Default)]
    pub struct UiState {}

    #[derive(Resource)]
    pub struct ApplicationCtx {
        /// The Ui's state in the Application.
        pub ui_mode: UiMode,

        pub ui_state: UiState,

        /// Startup initalized [`SmallRng`] random generator.
        /// Please note, that the [`SmallRng`] is insecure and should not be used in crypto contexts.
        pub rand: rand::rngs::SmallRng,

        pub server_instance_receiver: Receiver<anyhow::Result<ServerInstance>>,

        pub server_instance: Option<ServerInstance>,

        pub cancellation_token: CancellationToken,

        pub client_list: Arc<RwLock<Vec<RemoteGameClient>>>,

        pub tick_count: u64,
    }

    impl Default for ApplicationCtx {
        fn default() -> Self {
            Self {
                ui_mode: UiMode::MainMenu,
                ui_state: UiState::default(),
                rand: SmallRng::from_rng(&mut rand::rng()),
                server_instance_receiver: channel(255).1,
                server_instance: None,
                cancellation_token: CancellationToken::new(),
                client_list: Arc::new(RwLock::new(vec![])),
                tick_count: 0,
            }
        }
    }
}

pub mod client {
    use tokio::sync::mpsc::Sender;

    use bevy::ecs::system::Resource;

    use egui_toast::Toasts;

    use rand::{rngs::SmallRng, SeedableRng};
    use tokio::sync::mpsc::{channel, Receiver};
    use tokio_util::sync::CancellationToken;

    use crate::{networking::client::ClientConnection, UiMode};

    #[derive(Default)]
    pub struct UiState {
        pub connect_to_address: String,
    }

    #[derive(Resource)]
    pub struct ApplicationCtx {
        /// The Ui's mode in the Application.
        pub ui_mode: UiMode,

        /// The Ui's state in the Application,
        pub ui_state: UiState,

        /// Startup initalized [`SmallRng`] random generator.
        /// Please note, that the [`SmallRng`] is insecure and should not be used in crypto contexts.
        pub rand: rand::rngs::SmallRng,

        /// The Client's currently ongoing connection to a remote address.
        pub client_connection: Option<ClientConnection>,

        /// Receives the connecting threads connection result.
        pub connection_receiver: Receiver<anyhow::Result<ClientConnection>>,
        pub connection_sender: Sender<anyhow::Result<ClientConnection>>,

        /// Used to display notifications with egui
        pub egui_toasts: Toasts,

        pub cancellation_token: CancellationToken,
    }

    impl Default for ApplicationCtx {
        fn default() -> Self {
            let (connection_sender, connection_receiver) =
                channel::<anyhow::Result<ClientConnection>>(2000);

            Self {
                ui_mode: UiMode::MainMenu,
                ui_state: UiState::default(),
                client_connection: None,
                rand: SmallRng::from_rng(&mut rand::rng()),
                connection_receiver,
                connection_sender,
                egui_toasts: Toasts::new(),
                cancellation_token: CancellationToken::new(),
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum GameInput {
    MoveJump,
    MoveDuck,
    MoveRight,
    MoveLeft,
    Attack,
    
    Defend,

    Join,
    Exit,
}

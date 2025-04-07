pub mod game;
pub mod networking;

use bevy::ecs::system::Resource;
use networking::{IntermissionData, OngoingGameData};
use rand::{rngs::SmallRng, SeedableRng};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Deserialize, serde::Serialize)]
pub enum Direction {
    Left,
    #[default]
    Right,
    Up,
    Down,
}

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum UiLayer {
    Game(OngoingGameData),
    Intermission(IntermissionData),
    #[default]
    MainMenu,
    GameMenu,
    PauseWindow((PauseWindowState, Box<UiLayer>)),
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PauseWindowState {
    #[default]
    Main,
    Settings,
}

pub mod server {

    use bevy::{ecs::system::Resource, time::Timer};

    use rand::{rngs::SmallRng, SeedableRng};
    use tokio::sync::mpsc::{channel, Receiver};
    use tokio_util::sync::CancellationToken;

    use crate::{networking::server::ServerInstance, UiLayer};

    #[derive(Default)]
    pub struct UiState {}

    #[derive(Resource)]
    pub struct ApplicationCtx {
        /// The Ui's state in the Application.
        pub ui_mode: UiLayer,

        pub ui_state: UiState,

        /// Startup initalized [`SmallRng`] random generator.
        /// Please note, that the [`SmallRng`] is insecure and should not be used in crypto contexts.
        pub rand: rand::rngs::SmallRng,

        pub server_instance_receiver: Receiver<anyhow::Result<ServerInstance>>,

        pub server_instance: Option<ServerInstance>,

        pub cancellation_token: CancellationToken,

        pub tick_count: u64,

        pub intermission_timer: Option<Timer>,
        
        pub intermission_total_votes: usize,

        pub game_round_timer: Option<Timer>,
    }

    impl Default for ApplicationCtx {
        fn default() -> Self {
            Self {
                ui_mode: UiLayer::MainMenu,
                ui_state: UiState::default(),
                rand: SmallRng::from_rng(&mut rand::rng()),
                server_instance_receiver: channel(255).1,
                server_instance: None,
                cancellation_token: CancellationToken::new(),
                tick_count: 0,
                intermission_timer: None,
                game_round_timer: None,
                intermission_total_votes: 0,
            }
        }
    }
}

pub mod client {
    use std::path::PathBuf;

    use bevy_egui::egui::Rect;
    use tokio::sync::mpsc::Sender;

    use bevy::{asset::Handle, ecs::system::Resource, sprite::TextureAtlasLayout};

    use egui_toast::Toasts;

    use rand::{rngs::SmallRng, SeedableRng};
    use tokio::sync::mpsc::{channel, Receiver};
    use tokio_util::sync::CancellationToken;

    use crate::{networking::client::ClientConnection, UiLayer};

    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct UiState {
        pub connect_to_address: String,
        pub leaderboard_rect: Rect,
        pub current_resource_pack: Option<PathBuf>,
        pub username_buffer: String,
    }

    impl Default for UiState {
        fn default() -> Self {
            Self {
                connect_to_address: String::new(),
                username_buffer: String::new(),
                leaderboard_rect: Rect::NOTHING,
                current_resource_pack: None,
            }
        }
    }

    #[derive(Resource, serde::Serialize, serde::Deserialize)]
    #[serde(default)]
    pub struct ApplicationCtx {
        /// The Ui's layers in the Application.
        #[serde(skip)]
        pub ui_layer: UiLayer,

        /// The Ui's state in the Application,
        pub ui_state: UiState,

        /// Startup initalized [`SmallRng`] random generator.
        /// Please note, that the [`SmallRng`] is insecure and should not be used in crypto contexts.
        #[serde(skip)]
        pub rand: rand::rngs::SmallRng,

        /// The Client's currently ongoing connection to a remote address.
        #[serde(skip)]
        pub client_connection: Option<ClientConnection>,

        /// Receives the connecting threads connection result.
        #[serde(skip)]
        pub connection_receiver: Receiver<anyhow::Result<ClientConnection>>,
        #[serde(skip)]
        pub connection_sender: Sender<anyhow::Result<ClientConnection>>,

        /// Used to display notifications with egui
        #[serde(skip)]
        pub egui_toasts: Toasts,

        #[serde(skip)]
        pub cancellation_token: CancellationToken,

        pub settings: Settings,

        #[serde(skip)]
        pub texture_atlas_layouts: Handle<TextureAtlasLayout>,

        pub has_voted: bool,

        pub custom_textures: Option<CustomTexture>,
    }

    impl Default for ApplicationCtx {
        fn default() -> Self {
            let (connection_sender, connection_receiver) =
                channel::<anyhow::Result<ClientConnection>>(2000);

            Self {
                ui_layer: UiLayer::MainMenu,
                ui_state: UiState::default(),
                client_connection: None,
                rand: SmallRng::from_rng(&mut rand::rng()),
                connection_receiver,
                connection_sender,
                egui_toasts: Toasts::new(),
                cancellation_token: CancellationToken::new(),
                settings: Settings::default(),
                texture_atlas_layouts: Handle::<TextureAtlasLayout>::default(),
                has_voted: false,
                custom_textures: None,
            }
        }
    }

    #[derive(Debug, Default, Clone, serde::Deserialize, serde::Serialize)]
    pub struct Settings {
        pub fps: f64,
    }

    #[derive(Debug, Default, Clone, serde::Deserialize, serde::Serialize)]
    pub struct CustomTexture {
        pub walk: PathBuf,
        pub idle: PathBuf,
        pub attack: PathBuf,
        pub hurt: PathBuf,
        pub jump: PathBuf,
    }
}

/// This [`RandomEngine`] should never be used in crypto cases, as it uses a [`SmallRng`] in inside.
/// The struct has been purely created for making a Rng a [`Resource`] for bevy.
#[derive(Resource)]
pub struct RandomEngine {
    pub inner: SmallRng,
}

impl Default for RandomEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RandomEngine {
    pub fn new() -> Self {
        Self {
            inner: SmallRng::from_rng(&mut rand::rng()),
        }
    }
}

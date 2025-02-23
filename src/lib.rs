use bevy::{
    ecs::{component::Component, system::Resource},
    math::Vec2,
};
use tokio::net::{TcpStream, ToSocketAddrs};

#[derive(Component, Clone)]
pub struct SelfCharacter;

#[derive(Component, Clone)]
pub struct MapElement;


#[derive(Component, Clone)]
pub struct MapObject {
    size: Vec2,

    avoid_collision_from: Direction,
}

#[derive(Clone)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Resource)]
pub struct ClientConnection {
    tcp_stream: Option<TcpStream>,
}

impl Default for ClientConnection {
    fn default() -> Self {
        Self { tcp_stream: None }
    }
}

impl ClientConnection {
    pub async fn connect_to_address(addr: impl ToSocketAddrs) -> anyhow::Result<Self> {
        let tcp_stream = TcpStream::connect(addr).await?;

        Ok(Self {
            tcp_stream: Some(tcp_stream),
        })
    }
}

#[derive(Resource)]
pub struct ApplicationCtx {
    pub ui_state: UiState,
}

impl Default for ApplicationCtx {
    fn default() -> Self {
        Self {
            ui_state: UiState::MainMenu,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum UiState {
    Game,
    #[default]
    MainMenu,
    PauseWindow,
}

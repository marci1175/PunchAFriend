use bevy::{
    ecs::{component::Component, system::Resource},
    math::Vec2,
};
use bevy_rapier2d::prelude::{CollisionGroups, Group};
use tokio::net::{TcpStream, ToSocketAddrs};

#[derive(Component, Clone)]
pub struct SelfCharacter {
    pub can_jump: bool,
}

impl Default for SelfCharacter {
    fn default() -> Self {
        Self { can_jump: true }
    }
}

#[derive(Component, Clone, Default)]
pub struct ForeignCharacter;

#[derive(Component, Clone)]
pub struct MapElement;

#[derive(Component, Clone)]
pub struct AttackObject;

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

#[repr(u32)]
enum CollisionGroup {
    MapObject = 0b0001,      // 1
    SelfCharacter = 0b0010,  // 2
    ForeignCharacter = 0b0100, // 4
    AttackObj = 0b1000,      // 8
}

#[derive(Resource)]
pub struct CollisionGroupSet {
    /// Collides with all
    pub map_object: CollisionGroups,
    /// Only collides with MapObject & itself
    pub self_character: CollisionGroups,
    /// Collides with everything except SelfCharacter
    pub foreign_character: CollisionGroups,
    /// Collides with MapObject & ForeignCharacter, not SelfCharacter
    pub attack_obj: CollisionGroups,
}

impl CollisionGroupSet {
    pub fn new() -> Self {
        Self {
            map_object: CollisionGroups::new(
                Group::from_bits_truncate(CollisionGroup::MapObject as u32),
                Group::from_bits_truncate(0b1111), 
            ),
            self_character: CollisionGroups::new(
                Group::from_bits_truncate(CollisionGroup::SelfCharacter as u32),
                Group::from_bits_truncate(CollisionGroup::MapObject as u32 | CollisionGroup::SelfCharacter as u32 | CollisionGroup::ForeignCharacter as u32), 
            ),
            foreign_character: CollisionGroups::new(
                Group::from_bits_truncate(CollisionGroup::ForeignCharacter as u32),
                Group::from_bits_truncate(0b1111), 
            ),
            attack_obj: CollisionGroups::new(
                Group::from_bits_truncate(CollisionGroup::AttackObj as u32),
                Group::from_bits_truncate(CollisionGroup::MapObject as u32 | CollisionGroup::ForeignCharacter as u32), 
            ),
        }
    }
}
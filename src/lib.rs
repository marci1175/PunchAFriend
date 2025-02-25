use bevy::{
    ecs::{component::Component, system::Resource},
    math::Vec2,
    transform::components::Transform,
};
use bevy_rapier2d::prelude::{CollisionGroups, Group};
use rand::{rngs::SmallRng, SeedableRng};
use tokio::net::{TcpStream, ToSocketAddrs};

#[derive(Component, Clone)]
pub struct SelfCharacter {
    pub name: String,
    pub jumps_remaining: u8,
    pub direction: Direction,
}

impl Default for SelfCharacter {
    fn default() -> Self {
        Self {
            name: String::new(),
            jumps_remaining: 2,
            direction: Direction::default(),
        }
    }
}

impl SelfCharacter {
    pub fn new(name: String, jumps_remaining: u8, direction: Direction) -> Self {
        Self {
            name,
            jumps_remaining,
            direction,
        }
    }
}

#[derive(Component, Clone, Default)]
pub struct ForeignCharacter {
    pub effects: Vec<AttackEffects>,
}

#[derive(Component, Clone)]
pub struct MapElement;

#[derive(Component, Clone)]
pub struct AttackObject {
    pub attack_origin: Transform,
    pub attack_type: AttackType,
    pub attack_strength: f32,
}

impl AttackObject {
    pub fn new(attack_type: AttackType, attack_strength: f32, attack_origin: Transform) -> Self {
        Self {
            attack_origin,
            attack_type,
            attack_strength,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttackType {
    Directional(Direction),
    Super,
    Quick,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttackEffects {
    Slowdown,
    Disabled,
}

#[derive(Component, Clone)]
pub struct MapObject {
    size: Vec2,

    avoid_collision_from: Direction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Direction {
    Left,
    #[default]
    Right,
    Up,
    Down,
}

#[derive(Resource, Default)]
pub struct ClientConnection {
    tcp_stream: Option<TcpStream>,
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
    /// The Ui's state in the Application.
    pub ui_state: UiState,

    /// Startup initalized [`SmallRng`] random generator.
    /// Please note, that the [`SmallRng`] is insecure and should not be used in crypto contexts.
    pub rand: rand::rngs::SmallRng,
}

impl Default for ApplicationCtx {
    fn default() -> Self {
        Self {
            ui_state: UiState::MainMenu,
            rand: SmallRng::from_rng(&mut rand::rng()),
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
    MapObject = 0b0001,
    SelfCharacter = 0b0010,
    ForeignCharacter = 0b0100,
    AttackObj = 0b1000,
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

impl Default for CollisionGroupSet {
    fn default() -> Self {
        Self::new()
    }
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
                Group::from_bits_truncate(
                    CollisionGroup::MapObject as u32
                        | CollisionGroup::SelfCharacter as u32
                        | CollisionGroup::ForeignCharacter as u32,
                ),
            ),
            foreign_character: CollisionGroups::new(
                Group::from_bits_truncate(CollisionGroup::ForeignCharacter as u32),
                Group::from_bits_truncate(0b1111),
            ),
            attack_obj: CollisionGroups::new(
                Group::from_bits_truncate(CollisionGroup::AttackObj as u32),
                Group::from_bits_truncate(
                    CollisionGroup::MapObject as u32 | CollisionGroup::ForeignCharacter as u32,
                ),
            ),
        }
    }
}

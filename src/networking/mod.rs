use std::time::Duration;

use bevy::transform::components::Transform;
use bevy_rapier2d::prelude::Velocity;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::game::pawns::Player;

pub mod client;
pub mod server;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct RemoteClientRequest {
    pub id: Uuid,
    pub inputs: Vec<GameInput>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct RemoteServerRequest {
    pub id: Uuid,
    pub request: ServerRequest,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum ServerRequest {
    PlayerDisconnect,
    GameFlowControl(GameFlowControl),
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum GameFlowControl {
    Pause,
    Intermission(IntermissionData),
    OngoingGame,
}

/// This serves as all of the information necesarry for this intermission.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct IntermissionData {
    map_type: (),
    intermission_duration_left: Duration,
}

/// This server as a way for the server to send the state of an entity in the world.
/// This packet contains every necessary information about a player for the client to simulate it.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ServerTickUpdate {
    /// The position of the Entity in the tick.
    pub position: Transform,
    /// The velocity of the Entity, this is used so that the client can extrapolate the player's position via its physics engine. Please note that this is really inaccurate and extrapolation should be implemented.
    pub velocity: Velocity,
    /// Important information about the entitiy's [`Player`] instance.
    pub player: Player,
    /// The nth tick this packet was sent from.
    pub tick_count: u64,
}

impl ServerTickUpdate {
    pub fn new(position: Transform, velocity: Velocity, player: Player, tick_count: u64) -> Self {
        Self {
            position,
            velocity,
            player,
            tick_count,
        }
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ServerMetadata {
    pub client_uuid: Uuid,
    pub game_socket_port: u16,
}

impl ServerMetadata {
    pub fn new(client_uuid: Uuid, game_socket_port: u16) -> Self {
        Self {
            client_uuid,
            game_socket_port,
        }
    }
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, Copy)]
pub struct EndpointMetadata {
    pub game_socket_port: u16,
}

impl EndpointMetadata {
    pub fn new(game_socket_port: u16) -> Self {
        Self { game_socket_port }
    }

    pub fn into_server_metadata(&self, id: Uuid) -> ServerMetadata {
        ServerMetadata {
            game_socket_port: self.game_socket_port,
            client_uuid: id,
        }
    }
}

/// Writes a slice to a buffer with the slice's length as the header.
/// This results in the first 4 bytes being the [`u32`] representation of the slice's length.
pub async fn write_to_buf_with_len<T>(buf: &mut T, slice: &[u8]) -> anyhow::Result<()>
where
    T: AsyncWriteExt + Unpin,
{
    // Create the header.
    let mut slice_length = (slice.len() as u32).to_be_bytes().to_vec();

    // Extend the header with the slice so that it can be sent in 1 message
    slice_length.extend(slice);

    // Write the bytes to the buffer.
    buf.write_all(&slice_length).await?;

    Ok(())
}

pub const UDP_DATAGRAM_SIZE: usize = 65536;

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

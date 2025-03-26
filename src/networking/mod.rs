use std::time::Duration;

use bevy::{time::Timer, transform::components::Transform};
use bevy_rapier2d::prelude::Velocity;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::game::{
    map::{MapInstance, MapNameDiscriminants},
    pawns::Player,
};

pub mod client;
pub mod server;

/// This struct serves as a way to send a message by the clients, messages sent via the [`RemoteClientGameRequest`] are applied to the server's game world.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct RemoteClientGameRequest {
    /// The id of the client who has sent the message.
    /// IDs are handed out to the clients on connection.
    pub id: Uuid,
    /// The input of the clients connected to the server.
    /// Multiple inputs can be input at once.
    pub inputs: Vec<GameInput>,
}

/// This message type is used by the clients to send important information to the server.
/// *These messages should be sent thorugh TCP, as they contain critical information.*
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct RemoteClientRequest {
    /// The id of the client who has sent the message.
    /// IDs are handed out to the clients on connection.
    pub id: Uuid,
    /// The request sent by the client to the server.
    pub request: ClientRequest,
}

/// This message type is for the server to send critical information to the client.
/// Example: Map change, state updates (intermission, pause, etc.).
/// *These messages should be sent thorugh TCP, as they contain critical information.*
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct RemoteServerRequest {
    /// The inner request of the message.
    pub request: ServerRequest,
}

/// The types of messages which can be sent over in the [`RemoteServerRequest`] instance.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum ServerRequest {
    /// This message is sent if a user disconnects from the server.
    /// The server sends out this messaage to all of the clients to let them know that the user with the id (Inner value of this message) has disconnected.
    PlayerDisconnect(Uuid),
    /// This message is sent when the server wants to set a new state to the game.
    /// Example: Pause state, intermission, ...
    ServerGameStateControl(ServerGameState),
}

/// The types of GameStates which a server can request a client to enter.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum ServerGameState {
    /// Currently unused, may be used in the game to pause the match.
    Pause,
    /// Intermission state, in an intermission state clients can vote on the next map.
    Intermission(IntermissionData),
    /// Going game, this is sent if there is a game available to join immediately
    OngoingGame(MapInstance),
}

/// The types of messages a client can sent to control the server.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum ClientRequest {
    /// This message is sent if the game is currently in an intermission state, where players can vote on the next map.
    /// The inner value contain the name of the map the clients wants to vote on.
    Vote(String),
}

/// The message the server sends to all the clients, to share all the important information about the current intermission. ie.: Maps available for voting, duration of the intermission.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct IntermissionData {
    pub selectable_maps: Vec<MapNameDiscriminants>,
    pub intermission_duration_left: Timer,
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

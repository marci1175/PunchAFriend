use std::cmp::Ordering;

use bevy::transform::components::Transform;
use bevy_rapier2d::prelude::Velocity;
use chrono::{DateTime, Utc};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::game::{
    map::{MapInstance, MapNameDiscriminants, MapObjectUpdate},
    pawns::Pawn,
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

    PlayersStatisticsChange(Vec<ClientStatistics>),
}

/// The types of GameStates which a server can request a client to enter.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum ServerGameState {
    /// Currently unused, may be used in the game to pause the match.
    Pause,
    /// Intermission state, in an intermission state clients can vote on the next map.
    Intermission(IntermissionData),
    /// Ongoing game, this is sent if there is a game available to join immediately
    OngoingGame(OngoingGameData),
}

/// Contains all the information relating to this ongoing round's important data.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct OngoingGameData {
    /// Current map loaded
    pub current_map: MapInstance,
    /// Round end date
    pub round_end_date: DateTime<Utc>,
}

impl OngoingGameData {
    pub fn new(current_map: MapInstance, round_end_date: DateTime<Utc>) -> Self {
        Self {
            current_map,
            round_end_date,
        }
    }
}

/// The types of messages a client can sent to control the server.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum ClientRequest {
    /// This message is sent if the game is currently in an intermission state, where players can vote on the next map.
    /// The inner value contain the name of the map the clients wants to vote on.
    Vote(MapNameDiscriminants),
}

/// The message the server sends to all the clients, to share all the important information about the current intermission. ie.: Maps available for voting, duration of the intermission.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct IntermissionData {
    pub selectable_maps: Vec<(MapNameDiscriminants, usize)>,
    pub intermission_end_date: DateTime<Utc>,
}

impl IntermissionData {
    pub fn new(
        selectable_maps: Vec<(MapNameDiscriminants, usize)>,
        intermission_end_date: DateTime<Utc>,
    ) -> Self {
        Self {
            selectable_maps,
            intermission_end_date,
        }
    }
}

/// This server as a way for the server to send the state of an entity in the world.
/// This packet contains every necessary information about a player for the client to simulate it.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ServerTickUpdate {
    pub tick_update_type: TickUpdateType,
}

impl ServerTickUpdate {
    pub fn new(tick_update_type: TickUpdateType) -> Self {
        Self { tick_update_type }
    }
}

/// This server as a way for the server to send the state of an entity in the world.
/// This packet contains every necessary information about a player for the client to simulate it.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PawnUpdate {
    /// The position of the Entity in the tick.
    pub position: Transform,
    /// The velocity of the Entity, this is used so that the client can extrapolate the player's position via its physics engine. Please note that this is really inaccurate and extrapolation should be implemented.
    pub velocity: Velocity,
    /// Important information about the entitiy's [`Player`] instance.
    pub player: Pawn,
    /// The nth tick this packet was sent from.
    pub tick_count: u64,
}

impl PawnUpdate {
    pub fn new(position: Transform, velocity: Velocity, player: Pawn, tick_count: u64) -> Self {
        Self {
            position,
            velocity,
            player,
            tick_count,
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum TickUpdateType {
    Pawn(PawnUpdate),
    MapObject(MapObjectUpdate),
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

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct ConnectionMetadata {
    pub game_socket_port: u16,
}

impl ConnectionMetadata {
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

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct ClientMetadata {
    pub game_socket_port: u16,
    pub username: String,
}

impl ClientMetadata {
    pub fn new(game_socket_port: u16, username: String) -> Self {
        Self {
            game_socket_port,
            username,
        }
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

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize, Eq, Ord, Default)]
pub struct ClientStatistics {
    pub uuid: Uuid,
    pub username: String,
    pub kills: u32,
    pub deaths: u32,
    pub score: u32,
}

impl ClientStatistics {
    pub fn new(uuid: Uuid, username: String) -> Self {
        Self {
            uuid,
            username,
            ..Default::default()
        }
    }
}

impl PartialOrd for ClientStatistics {
    fn gt(&self, other: &Self) -> bool {
        self.kills > other.kills
    }

    fn lt(&self, other: &Self) -> bool {
        self.kills < other.kills
    }

    fn ge(&self, other: &Self) -> bool {
        self.kills >= other.kills
    }

    fn le(&self, other: &Self) -> bool {
        self.kills <= other.kills
    }

    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // First, compare by kills
        match self.kills.partial_cmp(&other.kills) {
            Some(Ordering::Equal) => {
                // If kills are equal, compare by score
                match self.score.partial_cmp(&other.score) {
                    Some(Ordering::Equal) => {
                        // If score is also equal, compare by deaths
                        match self.deaths.partial_cmp(&other.deaths) {
                            Some(Ordering::Equal) => {
                                // If deaths are equal, compare by uuid (as a last resort)
                                self.uuid.partial_cmp(&other.uuid)
                            }
                            other => other,
                        }
                    }
                    other => other,
                }
            }
            other => other,
        }
    }
}

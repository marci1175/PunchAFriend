use std::{net::SocketAddr, sync::Arc, time::Duration};

use bevy::{ecs::system::ResMut, time::Timer, transform::components::Transform};
use bevy_rapier2d::prelude::{
    ActiveEvents, AdditionalMassProperties, Ccd, Collider, KinematicCharacterController,
    LockedAxes, RigidBody, Velocity,
};
use bevy_tokio_tasks::TokioTasksRuntime;
use chrono::{Local, TimeDelta};
use dashmap::DashMap;
use parking_lot::{Mutex, RwLock};
use tokio::{
    io::AsyncReadExt,
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpListener, TcpSocket, TcpStream, UdpSocket,
    },
    select,
    sync::mpsc::{channel, Receiver, Sender},
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{
    game::{collision::CollisionGroupSet, map::MapInstance, pawns::Pawn},
    networking::{GameInput, RemoteClientRequest, UDP_DATAGRAM_SIZE},
};

use super::{
    write_to_buf_with_len, EndpointMetadata, OngoingGameData, RemoteClientGameRequest,
    RemoteServerRequest, ServerGameState, ServerMetadata, ServerRequest,
};

#[derive(Debug, Clone)]
pub struct RemoteGameClient {
    pub uid: Uuid,
    pub remote_game_socket_address: SocketAddr,
}

impl RemoteGameClient {
    pub fn new(uid: Uuid, remote_game_socket_address: SocketAddr) -> Self {
        Self {
            uid,
            remote_game_socket_address,
        }
    }
}

pub struct ServerInstance {
    pub tcp_listener: Arc<Mutex<TcpListener>>,
    pub udp_socket: Arc<UdpSocket>,

    pub metadata: EndpointMetadata,
    pub tcp_listener_port: u16,

    pub client_udp_receiver: Option<Receiver<(RemoteClientGameRequest, SocketAddr)>>,

    pub connected_client_game_sockets: Arc<DashMap<SocketAddr, (Uuid, Arc<Mutex<OwnedWriteHalf>>)>>,

    pub client_tcp_receiver: Option<Receiver<RemoteClientRequest>>,

    pub game_state: Arc<RwLock<ServerGameState>>,
}

impl ServerInstance {
    pub async fn create_server() -> anyhow::Result<Self> {
        let tcp_socket = TcpSocket::new_v6()?;

        tcp_socket.bind("[::]:0".parse()?)?;

        let tcp_listener = tcp_socket.listen(2048)?;

        let tcp_listener_port = tcp_listener.local_addr()?.port();

        let udp_socket = UdpSocket::bind("[::]:0").await?;

        let udp_socket_port = udp_socket.local_addr()?.port();

        let round_start_date = Local::now().to_utc();

        Ok(Self {
            tcp_listener: Arc::new(Mutex::new(tcp_listener)),
            udp_socket: Arc::new(udp_socket),
            tcp_listener_port,
            client_udp_receiver: None,
            metadata: EndpointMetadata::new(udp_socket_port),
            connected_client_game_sockets: Arc::new(DashMap::new()),
            client_tcp_receiver: None,
            game_state: Arc::new(RwLock::new(ServerGameState::OngoingGame(
                OngoingGameData::new(
                    MapInstance::map_flatground(),
                    round_start_date.checked_add_signed(TimeDelta::from_std(Duration::from_secs(8 * 60))?).unwrap()
                ),
            ))),
        })
    }
}

pub fn setup_remote_client_handler(
    server_instance: &mut ServerInstance,
    tokio_runtime: ResMut<TokioTasksRuntime>,
    cancellation_token: CancellationToken,
    collision_groups: CollisionGroupSet,
) {
    let tcp_listener = server_instance.tcp_listener.clone();

    let client_game_socket_list = server_instance.connected_client_game_sockets.clone();

    let (sender, receiver) = channel::<(RemoteClientGameRequest, SocketAddr)>(2000);
    let (tcp_sender, tcp_receiver) = channel::<RemoteClientRequest>(2000);

    let cancellation_token_clone = cancellation_token.clone();

    let udp_socket = server_instance.udp_socket.clone();

    let metadata = server_instance.metadata;

    let connected_clients_clone = client_game_socket_list.clone();

    let server_game_state = server_instance.game_state.clone();

    server_instance.client_tcp_receiver = Some(tcp_receiver);
    server_instance.client_udp_receiver = Some(receiver);

    // Spawn the incoming connection accepter thread
    tokio_runtime.spawn_background_task(move |mut ctx| async move {
        setup_client_listener(udp_socket.clone(), cancellation_token_clone.clone(), sender.clone(), connected_clients_clone.clone());
        
        loop {
            select! {
                _ = cancellation_token_clone.cancelled() => {
                    break;
                },

                Ok((tcp_stream, socket_addr)) = handle_incoming_request(tcp_listener.clone()) => {
                    // Create a new unique id for the connected client
                    let uuid = Uuid::new_v4();

                    let (mut read_half, mut write_half) = tcp_stream.into_split();

                    // Exchange metadata between client and server
                    if let Ok(client_metadata) = exchange_metadata(&mut read_half, &mut write_half, metadata.into_server_metadata(uuid)).await {
                        // Send the server's game state
                        let _ = send_request_to_client(&mut write_half, RemoteServerRequest { request: ServerRequest::ServerGameStateControl(server_game_state.read().clone()) }).await;

                        // Spawn a new entity for the connected client
                        ctx.run_on_main_thread(move |main_ctx| {
                            let mut worlds_commands = main_ctx.world.commands();

                            worlds_commands.spawn(RigidBody::Dynamic)
                            .insert(Collider::cuboid(20.0, 30.0))
                            .insert(Transform::from_xyz(0., 100., 0.))
                            .insert(ActiveEvents::COLLISION_EVENTS)
                            .insert(LockedAxes::ROTATION_LOCKED)
                            .insert(AdditionalMassProperties::Mass(0.1))
                            .insert(KinematicCharacterController {
                                apply_impulse_to_dynamic_bodies: false,
                                ..Default::default()
                            })
                            .insert(collision_groups.player)
                            .insert(Ccd::enabled())
                            .insert(Velocity::default())
                            .insert(Pawn::new_from_id(uuid)); 
                        }).await;

                        // Save the connected clients handle and ports
                        connected_clients_clone.insert(SocketAddr::new(socket_addr.ip(), client_metadata.game_socket_port), (uuid, Arc::new(Mutex::new(write_half))));
                        
                        // Try sending a made up client request to the server's client handler, so that if a client joins it will already send every information present for them even if theyre not moving.
                        sender.send((RemoteClientGameRequest {id: uuid, inputs: vec![GameInput::Join]}, socket_addr)).await.unwrap_or_default();
                        
                        // Clone the cancellation token
                        let cancellation_token_clone = cancellation_token_clone.clone();

                        // Clone the TcpSender
                        let tcp_sender = tcp_sender.clone();

                        // Create tcp listener
                        tokio::spawn(async move {
                            loop {
                                select! {
                                    _ = cancellation_token_clone.cancelled() => {
                                        break;
                                    }

                                    Ok(message_length) = read_half.read_u32() => {
                                        let mut buf = vec![0; message_length as usize];

                                        read_half.read_exact(&mut buf).await.unwrap();

                                        let message = rmp_serde::from_slice::<RemoteClientRequest>(&buf).unwrap();

                                        tcp_sender.send(message).await.unwrap();
                                    }
                                }
                            }
                        });
                    }
                }
            }
        }
    });
}

async fn handle_incoming_request(
    tcp_listener: Arc<Mutex<TcpListener>>,
) -> anyhow::Result<(TcpStream, SocketAddr)> {
    let client_connection = tcp_listener.lock().accept().await?;

    Ok(client_connection)
}

fn setup_client_listener(
    socket: Arc<UdpSocket>,
    cancellation_token: CancellationToken,
    client_request_channel: Sender<(RemoteClientGameRequest, SocketAddr)>,
    connected_clients: Arc<DashMap<SocketAddr, (Uuid, Arc<Mutex<OwnedWriteHalf>>)>>,
) {
    tokio::spawn(async move {
        loop {
            // Allocate the buffer for peeking the message's lenght
            let mut buf = vec![0; UDP_DATAGRAM_SIZE];

            select! {
                // Used to stop the server's processes
                _ = cancellation_token.cancelled() => {
                    // Break out of the loop if we have been signaled to do so
                    break;
                },

                // Peek the message's length
                read_result = socket.recv_from(&mut buf) => {
                    // Check the peek's result
                    match read_result {
                        Ok((_, address)) => {
                            // Check if the remote address has already been connected to the main server
                            if connected_clients.contains_key(&address) {
                                // Serialize the bytes from the message
                                if let Ok(client_request) = rmp_serde::from_slice::<RemoteClientGameRequest>(&buf[4..]) {
                                    // Send the message to the server's receiver
                                    client_request_channel.send((client_request, address)).await.unwrap();
                                }
                                else {
                                    println!("Received a message unsupported.");
                                }
                            }
                            else {
                                println!("Received a message from an unauthenticated account: {address}.");
                            }
                        }
                        Err(err) => {
                            // Print out error
                            dbg!(err);
                        }
                    }
                }
            }
        }
    });
}

async fn exchange_metadata(
    read_half: &mut OwnedReadHalf,
    write_half: &mut OwnedWriteHalf,
    metadata: ServerMetadata,
) -> anyhow::Result<EndpointMetadata> {
    let slice = rmp_serde::to_vec(&metadata)?;

    write_to_buf_with_len(write_half, &slice).await?;

    let metadata_length = read_half.read_u32().await?;

    let mut buf = vec![0; metadata_length as usize];

    read_half.read_exact(&mut buf).await?;

    let client_metadata = rmp_serde::from_slice::<EndpointMetadata>(&buf)?;

    Ok(client_metadata)
}

pub async fn notify_client_about_player_disconnect(
    write_half: &mut OwnedWriteHalf,
    uuid: Uuid,
) -> anyhow::Result<()> {
    let message = RemoteServerRequest {
        request: ServerRequest::PlayerDisconnect(uuid),
    };

    write_to_buf_with_len(write_half, &rmp_serde::to_vec(&message)?).await?;

    Ok(())
}

pub async fn send_request_to_client(
    tcp_stream: &mut OwnedWriteHalf,
    message: RemoteServerRequest,
) -> anyhow::Result<()> {
    write_to_buf_with_len(tcp_stream, &rmp_serde::to_vec(&message)?).await?;

    Ok(())
}

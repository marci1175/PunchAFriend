use std::{net::SocketAddr, sync::Arc};

use bevy::{ecs::system::ResMut, transform::components::Transform};
use bevy_rapier2d::prelude::{
    ActiveEvents, AdditionalMassProperties, Ccd, Collider, KinematicCharacterController,
    LockedAxes, RigidBody,
};
use bevy_tokio_tasks::TokioTasksRuntime;
use dashmap::DashMap;
use parking_lot::Mutex;
use tokio::{
    io::AsyncReadExt,
    net::{TcpListener, TcpSocket, TcpStream, UdpSocket},
    select,
    sync::mpsc::{channel, Receiver, Sender},
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{
    game::{collision::CollisionGroupSet, pawns::Player},
    networking::UDP_DATAGRAM_SIZE,
};

use super::{write_to_buf_with_len, EndpointMetadata, RemoteClientRequest, RemoteServerRequest, ServerMetadata, ServerRequest};

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

    pub server_receiver: Option<Receiver<(RemoteClientRequest, SocketAddr)>>,

    pub connected_client_game_sockets: Arc<DashMap<SocketAddr, (Uuid, TcpStream)>>,
}

impl ServerInstance {
    pub async fn create_server() -> anyhow::Result<Self> {
        let tcp_socket = TcpSocket::new_v6()?;

        tcp_socket.bind("[::]:0".parse()?)?;

        let tcp_listener = tcp_socket.listen(2048)?;

        let tcp_listener_port = tcp_listener.local_addr()?.port();

        let udp_socket = UdpSocket::bind("[::]:0").await?;

        let udp_socket_port = udp_socket.local_addr()?.port();

        Ok(Self {
            tcp_listener: Arc::new(Mutex::new(tcp_listener)),
            udp_socket: Arc::new(udp_socket),
            tcp_listener_port,
            server_receiver: None,
            metadata: EndpointMetadata::new(udp_socket_port),
            connected_client_game_sockets: Arc::new(DashMap::new()),
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

    let (sender, receiver) = channel::<(RemoteClientRequest, SocketAddr)>(2000);

    let cancellation_token_clone = cancellation_token.clone();

    let udp_socket = server_instance.udp_socket.clone();

    let metadata = server_instance.metadata;
    let connected_clients_clone = client_game_socket_list.clone();

    server_instance.server_receiver = Some(receiver);

    // Spawn the incoming connection accepter thread
    tokio_runtime.spawn_background_task(move |mut ctx| async move {        
        setup_client_listener(udp_socket.clone(), cancellation_token_clone.clone(), sender.clone(), connected_clients_clone.clone());
        
        loop {
            select! {
                _ = cancellation_token_clone.cancelled() => {
                    break;
                },

                Ok((mut tcp_stream, socket_addr)) = handle_incoming_request(tcp_listener.clone()) => {
                    let uuid = Uuid::new_v4();

                    if let Ok(client_metadata) = exchange_metadata(&mut tcp_stream, metadata.into_server_metadata(uuid)).await {
                        connected_clients_clone.insert(SocketAddr::new(socket_addr.ip(), client_metadata.game_socket_port), (uuid, tcp_stream));

                        // Spawn a new entity for the connected client
                        ctx.run_on_main_thread(move |main_ctx| {
                            let mut worlds_commands = main_ctx.world.commands();

                            worlds_commands.spawn(RigidBody::Dynamic)
                            .insert(Collider::ball(20.0))
                            .insert(Transform::from_xyz(0., 100., 0.))
                            .insert(LockedAxes::ROTATION_LOCKED)
                            .insert(AdditionalMassProperties::Mass(0.1))
                            .insert(KinematicCharacterController {
                                apply_impulse_to_dynamic_bodies: false,
                                ..Default::default()
                            })
                            .insert(ActiveEvents::COLLISION_EVENTS)
                            .insert(collision_groups.player)
                            .insert(Ccd::enabled())
                            .insert(Player::new_from_id(uuid)); 
                        }).await;
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
    client_request_channel: Sender<(RemoteClientRequest, SocketAddr)>,
    connected_clients: Arc<DashMap<SocketAddr, (Uuid, TcpStream)>>,
) {
    tokio::spawn(async move {
        loop {
            let mut buf = vec![0; UDP_DATAGRAM_SIZE];

            select! {
                _ = cancellation_token.cancelled() => {
                    break;
                },

                Ok((_, address)) = socket.peek_from(&mut buf) => {
                    if connected_clients.contains_key(&address) {
                        let incoming_msg_length = u32::from_be_bytes(buf[..4].try_into().unwrap());

                        let mut msg_buf = vec![0; incoming_msg_length as usize + 4];

                        match socket.recv(&mut msg_buf).await {
                            Ok(recv_size) => {
                                if recv_size != msg_buf.len() {
                                    panic!("Incoming message was too large. Exceeding UDP datagram size.");
                                }
                            },
                            Err(err) => {
                                // if an error occured remove the clients from the connected addresses
                                connected_clients.remove(&address);

                                // Print out the error
                                dbg!(err);

                                // Stop executing the remaining code
                                continue;
                            },
                        }

                        if let Ok(client_request) = rmp_serde::from_slice::<RemoteClientRequest>(&msg_buf[4..]) {
                            client_request_channel.send((client_request, address)).await.unwrap();
                        }
                        else {
                            panic!("Received a message unsupported");
                        }
                    }
                    else {
                        println!("Received a message from an unauthenticated account: {address}");
                    }
                }
            }
        }
    });
}

async fn exchange_metadata(
    tcp_stream: &mut TcpStream,
    metadata: ServerMetadata,
) -> anyhow::Result<EndpointMetadata> {
    let slice = rmp_serde::to_vec(&metadata)?;

    write_to_buf_with_len(tcp_stream, &slice).await?;

    let metadata_length = tcp_stream.read_u32().await?;

    let mut buf = vec![0; metadata_length as usize];

    tcp_stream.read_exact(&mut buf).await?;

    let client_metadata = rmp_serde::from_slice::<EndpointMetadata>(&buf)?;

    Ok(client_metadata)
}

pub async fn notify_client_about_player_disconnect(
    tcp_stream: &mut TcpStream,
    uuid: Uuid,
) -> anyhow::Result<()> {
    let message = RemoteServerRequest {
        id: uuid,
        request: ServerRequest::PlayerDisconnect,
    };

    write_to_buf_with_len(tcp_stream, &rmp_serde::to_vec(&message)?).await?;

    Ok(())
}

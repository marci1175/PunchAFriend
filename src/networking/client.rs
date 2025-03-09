use std::{net::SocketAddr, sync::Arc};

use bevy::ecs::system::Resource;
use tokio::{
    io::AsyncReadExt,
    net::{TcpStream, UdpSocket},
    select,
    sync::mpsc::{channel, Receiver, Sender},
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{
    networking::{RemoteClientRequest, ServerTickUpdate, UDP_DATAGRAM_SIZE},
    GameInput,
};

use super::{write_to_buf_with_len, EndpointMetadata, ServerMetadata};

#[derive(Resource)]
pub struct ClientConnection {
    pub server_metadata: ServerMetadata,

    pub sender_thread_handle: Sender<GameInput>,

    pub main_thread_handle: Receiver<ServerTickUpdate>,

    pub last_tick: u64,
}

impl ClientConnection {
    pub async fn connect_to_address(
        address: String,
        cancellation_token: CancellationToken,
    ) -> anyhow::Result<Self> {
        // Parse destination address.
        let dest_address: SocketAddr = address.parse()?;

        let mut tcp_stream = TcpStream::connect(dest_address).await?;

        // Create a new UdpSocket instance.
        // This is used to send ServerTicks to the client from the server.
        let udp_socket = Arc::new(UdpSocket::bind("[::]:0").await?);

        // Get the port the UdpSocket is bound to.
        // We will send this to the server so that it knows where to send the ticks to.
        let socket_port = udp_socket.local_addr()?.port();

        let client_metadata = EndpointMetadata::new(socket_port);

        // Exchange metadata with the server.
        // We will send the UdpSocket's port and the server will send our unique uuid, and the port of the Server's UdpSocket.
        let server_metadata = exchange_metadata(&mut tcp_stream, client_metadata).await?;

        // Connect to the destination address
        udp_socket
            .connect(dbg!(SocketAddr::new(
                dest_address.ip(),
                server_metadata.game_socket_port
            )))
            .await?;

        // Create a new channel pair
        let (sender, receiver) = channel::<GameInput>(2000);

        setup_server_sender(
            receiver,
            cancellation_token.clone(),
            udp_socket.clone(),
            server_metadata.client_uuid,
        )
        .await;

        let (client_sender, client_receiver) = channel::<ServerTickUpdate>(2000);

        setup_server_listener(cancellation_token, udp_socket, client_sender).await;

        Ok(ClientConnection {
            server_metadata,
            sender_thread_handle: sender,
            main_thread_handle: client_receiver,
            last_tick: 0,
        })
    }
}

pub async fn setup_server_sender(
    mut receiver: Receiver<GameInput>,
    cancellation_token: CancellationToken,
    udp_socket: Arc<UdpSocket>,
    client_uuid: Uuid,
) {
    tokio::spawn(async move {
        loop {
            select! {
                _ = cancellation_token.cancelled() => {
                    // Send the exit request to the server
                    send_game_action(udp_socket.clone(), GameInput::Exit, client_uuid).await;
                    
                    break;
                }

                Some(game_input) = receiver.recv() => {
                    send_game_action(udp_socket.clone(), game_input, client_uuid).await;
                }
            }
        }
    });
}

pub async fn setup_server_listener(
    cancellation_token: CancellationToken,
    socket: Arc<UdpSocket>,
    client_sender: Sender<ServerTickUpdate>,
) {
    tokio::spawn(async move {
        loop {
            let mut buf = vec![0; UDP_DATAGRAM_SIZE];

            select! {
                _ = cancellation_token.cancelled() => {
                    break;
                }

                Ok(_) = socket.peek(&mut buf) => {
                    let message_length = u32::from_be_bytes(buf[..4].try_into().unwrap());

                    let mut msg_buf = vec![0; message_length as usize + 4];

                    socket.recv(&mut msg_buf).await.unwrap();

                    let remote_client_request = rmp_serde::from_slice::<ServerTickUpdate>(&msg_buf[4..]).unwrap();

                    // This will return a SendError if the receiver is dropped before the select is completed.
                    let _ = client_sender.send(remote_client_request).await;
                }
            }
        }
    });
}

async fn exchange_metadata(
    tcp_stream: &mut TcpStream,
    client_metadata: EndpointMetadata,
) -> anyhow::Result<ServerMetadata> {
    // Allocate a buffer for the incoming message
    let mut msg_header_buf = vec![0; 4];

    // Read the bytes into the buffer
    tcp_stream.read_exact(&mut msg_header_buf).await?;

    // Allocate buffer
    let mut buf = vec![0; u32::from_be_bytes(msg_header_buf.try_into().unwrap()) as usize];

    // Write the bytes into the buffer
    tcp_stream.read_exact(&mut buf).await?;

    // Deserialize the bytes and return the result
    let server_metadata = rmp_serde::from_slice::<ServerMetadata>(&buf)?;

    // Serialize the client's metadata
    let metadata_bytes = rmp_serde::to_vec(&client_metadata)?;

    // Send the client's metadata
    write_to_buf_with_len(tcp_stream, &metadata_bytes).await?;

    Ok(server_metadata)
}

async fn send_game_action(send: Arc<UdpSocket>, game_input: GameInput, uuid: Uuid) {
    let message_bytes = rmp_serde::to_vec(&RemoteClientRequest {
        id: uuid,
        action: game_input,
    })
    .unwrap();

    let mut message_header = (message_bytes.len() as u32).to_be_bytes().to_vec();

    message_header.extend(message_bytes);

    send.writable().await.unwrap();

    send.send(&message_header).await.unwrap();
}

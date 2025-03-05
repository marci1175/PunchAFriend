use std::{net::SocketAddr, sync::Arc};

use bevy::ecs::system::Resource;
use quinn::{
    crypto::rustls::QuicClientConfig,
    rustls::{self},
    ClientConfig, Endpoint, RecvStream, SendStream,
};
use tokio::{
    select,
    sync::mpsc::{channel, Receiver, Sender},
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{
    networking::{RemoteClientRequest, ServerTickUpdate},
    GameInput,
};

use super::SkipServerVerification;

#[derive(Resource)]
pub struct ClientConnection {
    pub connection_handle: quinn::Endpoint,
    pub id: Uuid,
    pub sender_thread_handle: Sender<GameInput>,

    pub main_thread_handle: Receiver<ServerTickUpdate>,
}

impl ClientConnection {
    pub async fn connect_to_address(
        address: String,
        cancellation_token: CancellationToken,
    ) -> anyhow::Result<Self> {
        // Parse destination address.
        let dest_address: SocketAddr = address.parse()?;

        // Create a new QUIC instance.
        let mut endpoint = quinn::Endpoint::client("[::]:0".parse()?)?;

        endpoint.set_default_client_config(ClientConfig::new(Arc::new(
            QuicClientConfig::try_from(
                rustls::ClientConfig::builder()
                    .dangerous()
                    .with_custom_certificate_verifier(SkipServerVerification::new())
                    .with_no_client_auth(),
            )?,
        )));

        let (sender, receiver) = channel::<GameInput>(2000);

        let (mut send_stream, mut recv_stream) =
            open_connection_to_endpoint(endpoint.clone(), dest_address).await?;

        send_stream.write_all(&[0; 1]).await?;

        let uuid = fetch_metadata(&mut recv_stream).await?;

        setup_server_sender(receiver, cancellation_token.clone(), send_stream, uuid).await;

        let (client_sender, client_receiver) = channel::<ServerTickUpdate>(2000);

        setup_server_listener(cancellation_token, recv_stream, client_sender).await;

        Ok(ClientConnection {
            connection_handle: endpoint,
            id: uuid,
            sender_thread_handle: sender,
            main_thread_handle: client_receiver,
        })
    }
}

pub async fn setup_server_sender(
    mut receiver: Receiver<GameInput>,
    cancellation_token: CancellationToken,
    mut send_stream: SendStream,
    uuid: Uuid,
) {
    tokio::spawn(async move {
        loop {
            select! {
                _ = cancellation_token.cancelled() => {
                    break;
                }

                Some(game_input) = receiver.recv() => {
                    send_game_action(&mut send_stream, game_input, uuid).await;
                }
            }
        }
    });
}

pub async fn setup_server_listener(
    cancellation_token: CancellationToken,
    mut recv_stream: RecvStream,
    client_sender: Sender<ServerTickUpdate>,
) {
    tokio::spawn(async move {
        loop {
            let mut buf = vec![0; 4];

            select! {
                _ = cancellation_token.cancelled() => {
                    break;
                }

                Ok(_) = recv_stream.read_exact(&mut buf) => {
                    let message_length = u32::from_be_bytes(buf.try_into().unwrap());

                    let mut msg_buf = vec![0; message_length as usize];

                    recv_stream.read_exact(&mut msg_buf).await.unwrap();

                    let remote_client_request = rmp_serde::from_slice::<ServerTickUpdate>(&msg_buf).unwrap();

                    client_sender.send(remote_client_request).await.unwrap();
                }
            }
        }
    });
}

async fn open_connection_to_endpoint(
    endpoint: Endpoint,
    dest_address: SocketAddr,
) -> anyhow::Result<(quinn::SendStream, quinn::RecvStream)> {
    let connection_handle = endpoint.connect(dest_address, "punchafriend")?.await?;

    Ok(connection_handle.open_bi().await?)
}

async fn fetch_metadata(recv: &mut RecvStream) -> anyhow::Result<uuid::Uuid> {
    let mut buf = vec![0; 16];

    recv.read_exact(&mut buf).await?;

    let uuid = uuid::Uuid::from_bytes(
        buf.try_into()
            .map_err(|_| anyhow::Error::msg("Invalid UUID bytes in metadata."))?,
    );

    Ok(uuid)
}

async fn send_game_action(send: &mut SendStream, game_input: GameInput, uuid: Uuid) {
    let message_bytes = rmp_serde::to_vec(&RemoteClientRequest {
        id: uuid,
        action: game_input,
    })
    .unwrap();

    let message_length_bytes = (message_bytes.len() as u32).to_be_bytes();

    send.write_all(&message_length_bytes).await.unwrap();
    send.write_all(&message_bytes).await.unwrap();
}

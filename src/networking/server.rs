use std::{net::SocketAddr, sync::Arc};

use bevy::ecs::{entity::Entity, system::{ResMut, Resource}};
use bevy_tokio_tasks::TokioTasksRuntime;
use parking_lot::{Mutex, RwLock};
use quinn::{
    rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer}, Endpoint, RecvStream, SendStream, ServerConfig
};
use tokio::{select, sync::broadcast::{channel, Sender}};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{game::pawns::Player, GameAction};

pub struct RemoteClient {
    pub uid: Uuid,
    pub send_stream_handle: SendStream,
}

impl RemoteClient {
    pub fn new(uid: Uuid, send_stream_handle: SendStream) -> Self {
        Self { uid, send_stream_handle }
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct RemoteClientRequest {
    id: Uuid,
    action: GameAction,
}

#[derive(Clone)]
pub struct ServerInstance {
    pub connection_handle: quinn::Endpoint,

    pub local_address: SocketAddr,

    pub certificate: CertificateDer<'static>,

    pub connected_endpoints: Arc<RwLock<Vec<RemoteClient>>>,
}

impl ServerInstance {
    pub async fn create_server() -> anyhow::Result<Self> {
        let (config, cert) = configure_server()?;

        let quic_endpoint = Endpoint::server(config, "[::]:0".parse()?)?;

        let local_addr = quic_endpoint.local_addr()?;

        Ok(
            Self {
                connection_handle: quic_endpoint,
                local_address: local_addr,
                certificate: cert,
                connected_endpoints: Arc::new(RwLock::new(vec![])),
            }
        )
    }
}

fn configure_server() -> anyhow::Result<(ServerConfig, CertificateDer<'static>)> {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();

    let cert_der = CertificateDer::from(cert.cert);

    let priv_key = PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der());

    let mut server_config =
        ServerConfig::with_single_cert(vec![cert_der.clone()], priv_key.into())?;

    let transport_config = Arc::get_mut(&mut server_config.transport).unwrap();

    transport_config.max_concurrent_uni_streams(0_u8.into());

    Ok((server_config, cert_der))
}

pub fn setup_remote_client(server_instance: ServerInstance, tokio_runtime: ResMut<TokioTasksRuntime>, cancellation_token: CancellationToken) {
    let server_endpoint = server_instance.connection_handle.clone();
    
    let client_list = server_instance.connected_endpoints.clone();

    let (sender, mut receiver) = channel::<RemoteClientRequest>(2000);

    let cancellation_token_clone = cancellation_token.clone();

    // Spawn the incoming connection accepter thread
    tokio_runtime.spawn_background_task(|mut _ctx| async move {        
        loop {
            select! {
                _ = cancellation_token_clone.cancelled() => {
                    break;
                },

                Ok((mut send, recv)) = handle_incoming_request(server_endpoint.clone()) => {
                    let uuid = Uuid::new_v4();

                    if let Ok(_) = send_metadata(&mut send, uuid).await {
                        client_list.write().push(RemoteClient::new(uuid, send));

                        setup_client_handler(recv, cancellation_token_clone.clone(), sender.clone());
                    }
                }
            }
        }
    });

    tokio_runtime.spawn_background_task(|mut ctx| async move {
        loop {
            select! {
                _ = cancellation_token.cancelled() => {
                    break;
                },

                Ok(remote_client_request) = receiver.recv() => {
                    // Access the main thread from the async thread
                    ctx.run_on_main_thread(move |mut main_ctx| {
                        // Create a query from the world in the main thread
                        let mut query = main_ctx.world.query::<(Entity, &mut Player)>();
                    
                        // iterate over all the query results
                        for (entity, player) in query.iter_mut(&mut main_ctx.world) {
                            // This is the remote client's pawn.
                            if player.id == remote_client_request.id {

                            }
                        }
                    }).await;
                }
            }
        }
    });
}

async fn handle_incoming_request(server_endpoint: Endpoint) -> anyhow::Result<(quinn::SendStream, quinn::RecvStream)> {
    let client_connection = server_endpoint.accept().await.ok_or(anyhow::Error::msg("Client has closed the connection before it could succeed."))?;

    let connection = client_connection.accept()?.await?;
    
    Ok(connection.accept_bi().await?)
}

fn setup_client_handler(mut recv_stream: RecvStream, cancellation_token: CancellationToken, client_request_channel: Sender<RemoteClientRequest>) {
    tokio::spawn(async move {
        loop {
            let mut buf = vec![0; 4];

            select! {
                _ = cancellation_token.cancelled() => {
                    break;
                },

                _ = recv_stream.read_exact(&mut buf) => {
                    let incoming_msg_length = u32::from_be_bytes(buf.try_into().unwrap());

                    let mut msg_buf = vec![0; incoming_msg_length as usize];

                    recv_stream.read_exact(&mut msg_buf).await.unwrap();

                    if let Ok(client_request) = rmp_serde::from_slice::<RemoteClientRequest>(&msg_buf) {
                        client_request_channel.send(client_request).unwrap();
                    }
                    else {
                        panic!("Received a message unsupported");
                    }
                } 
            }
        }
    });
}

async fn send_metadata(send: &mut SendStream, uuid: Uuid) -> anyhow::Result<()> {
    send.write_all(uuid.as_bytes()).await?;

    Ok(())
}
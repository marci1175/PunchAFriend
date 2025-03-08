use std::{net::SocketAddr, sync::Arc};

use bevy::transform::components::Transform;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::{game::pawns::Player, GameInput};

pub mod client;
pub mod server;

#[derive(Debug)]
struct SkipServerVerification(Arc<rustls::crypto::CryptoProvider>);

impl SkipServerVerification {
    fn new() -> Arc<Self> {
        Arc::new(Self(Arc::new(rustls::crypto::ring::default_provider())))
    }
}

impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct RemoteClientRequest {
    pub id: Uuid,
    pub action: GameInput,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ServerTickUpdate {
    pub transfrom: Transform,
    pub player: Player,
}

impl ServerTickUpdate {
    pub fn new(position: Transform, player: Player) -> Self {
        Self {
            transfrom: position,
            player,
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
        Self { client_uuid, game_socket_port }
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
pub async fn write_to_buf_with_len<T> (buf: &mut T, slice: &[u8]) -> anyhow::Result<()>
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
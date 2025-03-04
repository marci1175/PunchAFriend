use std::{net::SocketAddr, sync::Arc};

use bevy::ecs::system::Resource;
use quinn::{
    rustls::{self, pki_types::CertificateDer},
    ClientConfig,
};

#[derive(Resource)]
pub struct ClientConnection {
    pub connection_handle: quinn::Endpoint,
}

impl ClientConnection {
    pub async fn connect_to_address(
        address: String,
        certificate: CertificateDer<'static>,
    ) -> anyhow::Result<Self> {
        // Parse socket address.
        let address: SocketAddr = address.parse()?;

        // Create a new QUIC instance.
        let mut quic_stream = quinn::Endpoint::client(address)?;

        // Create the new Certificate variable
        let mut certs = rustls::RootCertStore::empty();

        certs.add(certificate)?;

        quic_stream
            .set_default_client_config(ClientConfig::with_root_certificates(Arc::new(certs))?);

        Ok(ClientConnection {
            connection_handle: quic_stream,
        })
    }
}

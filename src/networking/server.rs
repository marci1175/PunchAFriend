use std::{net::SocketAddr, sync::Arc};

use bevy::ecs::system::Resource;
use quinn::{
    rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer},
    Endpoint, ServerConfig,
};

#[derive(Resource)]
pub struct ServerInstance {
    pub connection_handle: quinn::Endpoint,
}

impl ServerInstance {
    pub async fn create_server() -> anyhow::Result<(Self, CertificateDer<'static>, SocketAddr)> {
        let (config, cert) = configure_server()?;

        let quic_endpoint = Endpoint::server(config, "[::]:0".parse()?)?;

        let local_addr = quic_endpoint.local_addr()?;

        Ok((
            Self {
                connection_handle: quic_endpoint,
            },
            cert,
            local_addr,
        ))
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

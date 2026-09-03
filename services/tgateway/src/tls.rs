use std::io::ErrorKind;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use kim_core::{
    Acceptor, Error, MailboxFullHook, MessageListener, Server, StateListener, WriteFullPolicy,
};
use kim_tcp::{acquire_permit, apply_socket_opts, serve_conn, FrontendState, TcpConn, TcpServer};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::ServerConfig;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tracing::{info, warn};

const DEFAULT_HANDSHAKE_WAIT: Duration = Duration::from_secs(10);

pub fn load_tls(cert: &str, key: &str) -> Result<Option<TlsAcceptor>, Box<dyn std::error::Error>> {
    if cert.trim().is_empty() || key.trim().is_empty() {
        return Ok(None);
    }
    let _ = rustls::crypto::ring::default_provider().install_default();
    let certs = load_certs(Path::new(cert))?;
    let key = load_key(Path::new(key))?;
    let cfg = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;
    Ok(Some(TlsAcceptor::from(Arc::new(cfg))))
}

fn load_certs(path: &Path) -> Result<Vec<CertificateDer<'static>>, std::io::Error> {
    let mut reader = std::io::BufReader::new(std::fs::File::open(path)?);
    rustls_pemfile::certs(&mut reader).collect::<Result<Vec<_>, _>>()
}

fn load_key(path: &Path) -> Result<PrivateKeyDer<'static>, std::io::Error> {
    let mut reader = std::io::BufReader::new(std::fs::File::open(path)?);
    rustls_pemfile::private_key(&mut reader)?.ok_or_else(|| {
        std::io::Error::new(ErrorKind::InvalidData, "tls key pem has no private key")
    })
}

pub struct TlsFrontend {
    local_addr: std::net::SocketAddr,
    state: Arc<FrontendState>,
    acceptor: TlsAcceptor,
    handshake_wait: Duration,
}

impl TlsFrontend {
    pub fn wrap(
        server: TcpServer,
        acceptor: TlsAcceptor,
        handshake_wait: Option<Duration>,
    ) -> Self {
        Self {
            local_addr: server.local_addr(),
            state: server.into_frontend_state(),
            acceptor,
            handshake_wait: handshake_wait.unwrap_or(DEFAULT_HANDSHAKE_WAIT),
        }
    }

    async fn accept_loop(&self, listener: TcpListener) {
        loop {
            tokio::select! {
                _ = self.state.shutdown_notify().notified() => {
                    info!("tls frontend shutting down");
                    break;
                }
                accepted = listener.accept() => {
                    let (mut stream, peer) = match accepted {
                        Ok(v) => v,
                        Err(err) => {
                            warn!(%err, "accept failed");
                            continue;
                        }
                    };
                    if self.state.is_closed() {
                        continue;
                    }
                    // Plaintext accept uses TcpConn::new, which sets nodelay. TLS wraps
                    // the socket before that, so Nagle would otherwise delay small frames.
                    if let Err(err) = stream.set_nodelay(true) {
                        warn!(%err, %peer, "tcp nodelay failed");
                    }
                    if let Err(err) = apply_socket_opts(&stream, &self.state.socket_opts()) {
                        warn!(%err, %peer, "socket opts failed");
                    }
                    let permit = match acquire_permit(self.state.connection_limit().as_ref(), &mut stream).await {
                        Ok(p) => p,
                        Err(()) => continue,
                    };
                    let acceptor = self.acceptor.clone();
                    let ctx = self.state.serve_ctx();
                    let handshake_wait = self.handshake_wait;
                    self.state.spawn_conn(async move {
                        let _permit = permit;
                        match tokio::time::timeout(handshake_wait, acceptor.accept(stream)).await {
                            Ok(Ok(tls)) => {
                                let conn = TcpConn::with_peer(tls, Some(peer.to_string()));
                                if let Err(err) = serve_conn(conn, ctx).await {
                                    warn!(%peer, %err, "connection ended");
                                }
                            }
                            Ok(Err(err)) => warn!(%err, %peer, "tls handshake failed"),
                            Err(_) => warn!(%peer, "tls handshake timeout"),
                        }
                    }).await;
                }
            }
        }
    }
}

#[async_trait]
impl Server for TlsFrontend {
    fn set_acceptor(&mut self, acceptor: Arc<dyn Acceptor>) {
        self.state.set_acceptor(acceptor);
    }

    fn set_message_listener(&mut self, listener: Arc<dyn MessageListener>) {
        self.state.set_message_listener(listener);
    }

    fn set_state_listener(&mut self, listener: Arc<dyn StateListener>) {
        self.state.set_state_listener(listener);
    }

    fn set_read_wait(&mut self, wait: Duration) {
        self.state.set_read_wait(wait);
    }

    fn set_write_full(&mut self, policy: WriteFullPolicy) {
        self.state.set_write_full(policy);
    }

    fn set_on_mailbox_full(&mut self, hook: MailboxFullHook) {
        self.state.set_on_mailbox_full(hook);
    }

    async fn start(&self) -> Result<(), Error> {
        let listener = self
            .state
            .take_listener()
            .await
            .ok_or_else(|| Error::other("tls frontend requires the listener"))?;
        if self.state.is_closed() {
            info!("tls frontend already shut down");
            return Ok(());
        }
        info!(local = %self.local_addr, "tls frontend listening");
        self.accept_loop(listener).await;
        Ok(())
    }

    async fn push(&self, channel_id: &str, payload: Bytes) -> Result<(), Error> {
        self.state.push(channel_id, payload).await
    }

    async fn close_channel(&self, channel_id: &str) -> Result<(), Error> {
        self.state.close_channel(channel_id).await
    }

    async fn shutdown(&self) -> Result<(), Error> {
        self.state.shutdown().await
    }
}

#[cfg(test)]
mod tests {
    use super::load_tls;

    #[test]
    fn empty_paths_are_plaintext() {
        assert!(load_tls("", "").unwrap().is_none());
        assert!(load_tls("  ", "/tmp/x").unwrap().is_none());
        assert!(load_tls("/tmp/x", "").unwrap().is_none());
    }

    #[derive(serde::Deserialize, Default)]
    struct Extra {
        #[serde(default)]
        tls_cert: String,
        #[serde(default)]
        tls_key: String,
        #[serde(default)]
        max_connections: Option<usize>,
    }

    #[test]
    fn deploy_sample_configs_load() {
        let deploy = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../deploy");
        for name in ["tgateway.toml", "tgateway-tls-off.toml"] {
            let path = deploy.join(name);
            gateway::load_config(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
            let extra: Extra = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            assert_eq!(extra.max_connections, Some(10000), "{name}");
            if name.contains("tls-off") {
                assert!(extra.tls_cert.is_empty(), "{name}");
                assert!(extra.tls_key.is_empty(), "{name}");
            } else {
                assert!(extra.tls_cert.contains("tgateway.pem"), "{name}");
                assert!(extra.tls_key.contains("tgateway-key.pem"), "{name}");
            }
        }
    }
}

//! TCP socket options shared by `kim-tcp` and `kim-ws` client dials.

use std::io;
use std::time::Duration;

use socket2::{SockRef, TcpKeepalive};
use tokio::net::TcpStream;

/// `None` = do not set keepalive.
#[derive(Clone, Debug)]
pub struct SocketOpts {
    pub keepalive: Option<Keepalive>,
}

#[derive(Clone, Copy, Debug)]
pub struct Keepalive {
    pub idle: Duration,
    pub interval: Duration,
    pub retries: u32,
}

impl Default for Keepalive {
    fn default() -> Self {
        Self {
            idle: Duration::from_secs(30),
            interval: Duration::from_secs(10),
            retries: 3,
        }
    }
}

impl Default for SocketOpts {
    fn default() -> Self {
        Self {
            keepalive: Some(Keepalive::default()),
        }
    }
}

impl SocketOpts {
    pub fn apply(&self, sock: &SockRef<'_>) -> io::Result<()> {
        let Some(ka) = self.keepalive else {
            return Ok(());
        };
        let mut tcp_ka = TcpKeepalive::new()
            .with_time(ka.idle)
            .with_interval(ka.interval);
        #[cfg(any(
            target_os = "android",
            target_os = "freebsd",
            target_os = "ios",
            target_os = "linux",
            target_os = "macos"
        ))]
        {
            tcp_ka = tcp_ka.with_retries(ka.retries);
        }
        sock.set_tcp_keepalive(&tcp_ka)
    }
}

pub fn apply_socket_opts(stream: &TcpStream, opts: &SocketOpts) -> io::Result<()> {
    opts.apply(&SockRef::from(stream))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn apply_socket_opts_on_loopback_does_not_panic() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let client = TcpStream::connect(addr).await.unwrap();
        let (server, _) = listener.accept().await.unwrap();
        apply_socket_opts(&client, &SocketOpts::default()).unwrap();
        apply_socket_opts(&server, &SocketOpts { keepalive: None }).unwrap();
        let _ = client.set_nodelay(true);
    }
}

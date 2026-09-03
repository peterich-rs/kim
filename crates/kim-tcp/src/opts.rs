use std::io;
use std::time::Duration;

use socket2::{SockRef, TcpKeepalive};

#[derive(Clone, Debug)]
pub struct SocketOpts {
    /// `None` = 不设 keepalive。
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

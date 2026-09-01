//! Process shutdown: SIGTERM + SIGINT on unix, ctrl_c elsewhere.

/// Wait until the process should drain (K8s/Compose SIGTERM, or Ctrl-C).
pub async fn wait_shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut term = match signal(SignalKind::terminate()) {
            Ok(s) => s,
            Err(err) => {
                tracing::error!(%err, "listen SIGTERM");
                std::future::pending::<()>().await;
                return;
            }
        };
        let mut int = match signal(SignalKind::interrupt()) {
            Ok(s) => s,
            Err(err) => {
                tracing::error!(%err, "listen SIGINT");
                std::future::pending::<()>().await;
                return;
            }
        };
        tokio::select! {
            _ = term.recv() => {}
            _ = int.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::wait_shutdown_signal;
    use std::time::Duration;

    #[tokio::test]
    async fn sigterm_unblocks_wait() {
        let wait = tokio::spawn(wait_shutdown_signal());
        tokio::time::sleep(Duration::from_millis(50)).await;
        let pid = std::process::id().to_string();
        let status = std::process::Command::new("kill")
            .args(["-TERM", &pid])
            .status()
            .expect("kill");
        assert!(status.success());
        tokio::time::timeout(Duration::from_secs(2), wait)
            .await
            .expect("SIGTERM should complete wait_shutdown_signal")
            .expect("join");
    }
}

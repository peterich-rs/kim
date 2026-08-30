use std::sync::{Mutex, OnceLock};

use kim_client::{ClientConfig, KimClient};
use tokio::runtime::Runtime;

fn rt() -> &'static Runtime {
    static RT: OnceLock<Runtime> = OnceLock::new();
    RT.get_or_init(|| Runtime::new().expect("tokio runtime"))
}

/// Opaque handle. UI is a shell; session/login/talk live here.
pub struct KimApi {
    inner: Mutex<KimClient>,
}

impl KimApi {
    #[flutter_rust_bridge::frb(sync)]
    pub fn new(url: String, token: String) -> Self {
        Self {
            inner: Mutex::new(KimClient::new(ClientConfig::new(url, token))),
        }
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn connect(&self) -> Result<String, String> {
        let mut c = self.inner.lock().map_err(|e| e.to_string())?;
        rt().block_on(c.connect()).map_err(|e| e.to_string())?;
        Ok(format!("connected {}", c.url()))
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn login(&self) -> Result<String, String> {
        let mut c = self.inner.lock().map_err(|e| e.to_string())?;
        let s = rt().block_on(c.login()).map_err(|e| e.to_string())?;
        Ok(format!("channel_id={} account={}", s.channel_id, s.account))
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn ping(&self) -> Result<String, String> {
        let c = self.inner.lock().map_err(|e| e.to_string())?;
        rt().block_on(c.ping()).map_err(|e| e.to_string())?;
        Ok("pong".into())
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn talk_to_user(&self, dest: String, body: String) -> Result<String, String> {
        let c = self.inner.lock().map_err(|e| e.to_string())?;
        let r = rt()
            .block_on(c.talk_to_user(&dest, &body))
            .map_err(|e| e.to_string())?;
        Ok(format!("message_id={} send_time={}", r.message_id, r.send_time))
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn disconnect(&self) -> Result<String, String> {
        let mut c = self.inner.lock().map_err(|e| e.to_string())?;
        rt().block_on(c.disconnect()).map_err(|e| e.to_string())?;
        Ok("disconnected".into())
    }
}

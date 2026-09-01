#[cfg(feature = "consul")]
mod consul;
mod naming;
mod registration;
mod static_naming;

#[cfg(feature = "consul")]
pub use consul::ConsulNaming;
pub use naming::{Error, Naming};
pub use registration::DefaultRegistration;
pub use static_naming::StaticNaming;

use std::sync::Arc;

/// StaticNaming when `consul_http_addr` is empty. ConsulNaming when set
/// (requires the `consul` feature).
pub fn open_naming(
    consul_http_addr: Option<&str>,
    static_regs: Vec<DefaultRegistration>,
) -> Result<Arc<dyn Naming>, Error> {
    match consul_http_addr {
        None | Some("") => Ok(Arc::new(StaticNaming::from_slice(static_regs))),
        Some(url) => open_consul(url),
    }
}

#[cfg(feature = "consul")]
fn env_nonempty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(feature = "consul")]
fn open_consul(url: &str) -> Result<Arc<dyn Naming>, Error> {
    let token = env_nonempty("CONSUL_HTTP_TOKEN");
    let ca = match env_nonempty("CONSUL_CACERT") {
        Some(path) => {
            Some(std::fs::read_to_string(&path).map_err(|e| Error::Other(format!("{path}: {e}")))?)
        }
        None => None,
    };
    let identity = match (
        env_nonempty("CONSUL_CLIENT_CERT"),
        env_nonempty("CONSUL_CLIENT_KEY"),
    ) {
        (Some(cert), Some(key)) => {
            let mut pem = std::fs::read(&cert).map_err(|e| Error::Other(format!("{cert}: {e}")))?;
            pem.push(b'\n');
            pem.extend(std::fs::read(&key).map_err(|e| Error::Other(format!("{key}: {e}")))?);
            Some(pem)
        }
        (None, None) => None,
        _ => {
            return Err(Error::Other(
                "CONSUL_CLIENT_CERT and CONSUL_CLIENT_KEY must both be set".into(),
            ));
        }
    };
    Ok(Arc::new(ConsulNaming::connect(
        url,
        token.as_deref(),
        ca.as_deref(),
        identity.as_deref(),
    )?))
}

#[cfg(not(feature = "consul"))]
fn open_consul(_url: &str) -> Result<Arc<dyn Naming>, Error> {
    Err(Error::Other("rebuild with --features consul".into()))
}

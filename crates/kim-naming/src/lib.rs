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
fn open_consul(url: &str) -> Result<Arc<dyn Naming>, Error> {
    Ok(Arc::new(ConsulNaming::new(url)?))
}

#[cfg(not(feature = "consul"))]
fn open_consul(_url: &str) -> Result<Arc<dyn Naming>, Error> {
    Err(Error::Other("rebuild with --features consul".into()))
}

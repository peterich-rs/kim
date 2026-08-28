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

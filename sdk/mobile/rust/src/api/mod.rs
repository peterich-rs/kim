pub mod auth;
pub mod client;
pub mod simple;

use std::sync::OnceLock;

use tokio::runtime::Runtime;

pub(crate) fn rt() -> &'static Runtime {
    static RT: OnceLock<Runtime> = OnceLock::new();
    RT.get_or_init(|| Runtime::new().expect("tokio runtime"))
}

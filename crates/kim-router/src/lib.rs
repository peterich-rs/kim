//! 指令路由：Handler、Context（Resp / Dispatch 按网关合包）、SessionStorage trait。

mod context;
mod dispatcher;
mod location;
mod router;
mod storage;

pub use context::Context;
pub use dispatcher::{Dispatcher, RouterError};
pub use location::Location;
pub use router::{HandlerFn, Router};
pub use storage::{SessionError, SessionStorage};

#[cfg(any(test, feature = "test-util"))]
pub mod test_support;

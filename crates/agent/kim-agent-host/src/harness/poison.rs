use std::sync::Once;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostPoison {
    MachinePanic,
    CancelGrace,
}

impl HostPoison {
    pub fn message(self) -> &'static str {
        match self {
            Self::MachinePanic => "machine panic",
            Self::CancelGrace => "cancel_grace exceeded",
        }
    }
}

pub fn install_panic_hook() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            tracing::error!(target: "harness.panic", "agent host panic");
            prev(info);
        }));
    });
}

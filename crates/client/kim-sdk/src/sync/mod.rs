//! Persist-then-ack hook is installed after the store attach path exists.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnreadPolicy {
    Keep,
    IfInserted,
}

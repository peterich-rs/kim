use crate::ids::SessionEpoch;

#[async_trait::async_trait]
pub trait AgentPort: Send + Sync {
    async fn enqueue_turn(&self, dest: &str, text: &str, in_reply_to: i64, epoch: SessionEpoch);
    async fn catch_up(&self, dests: &[String], epoch: SessionEpoch);
}

pub struct NoopAgent;

#[async_trait::async_trait]
impl AgentPort for NoopAgent {
    async fn enqueue_turn(
        &self,
        _dest: &str,
        _text: &str,
        _in_reply_to: i64,
        _epoch: SessionEpoch,
    ) {
    }

    async fn catch_up(&self, _dests: &[String], _epoch: SessionEpoch) {}
}

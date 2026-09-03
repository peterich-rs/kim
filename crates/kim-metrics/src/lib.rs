//! Prometheus registry + text HTTP. Used by examples only.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::Router;
use prometheus::{
    Encoder, GaugeVec, HistogramOpts, HistogramVec, IntCounterVec, IntGaugeVec, Opts, Registry,
    TextEncoder,
};
use thiserror::Error;

const COMMANDS: &[&str] = &[
    "login.signin",
    "login.signout",
    "login.renew",
    "chat.demo.echo",
    "chat.user.talk",
    "chat.group.talk",
    "chat.group.create",
    "chat.group.join",
    "chat.group.quit",
    "chat.group.detail",
    "chat.group.members",
    "chat.talk.ack",
    "chat.offline.index",
    "chat.offline.content",
    "chat.user.profile",
    "chat.user.update",
    "chat.user.search",
    "chat.friend.request",
    "chat.friend.accept",
    "chat.friend.reject",
    "chat.friend.remove",
    "chat.friend.list",
    "chat.friend.incoming",
    "chat.block.add",
    "chat.block.remove",
    "chat.block.list",
    "chat.inbox.list",
    "chat.inbox.read",
    "chat.history",
];

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Other(String),
}

pub struct KimMetrics {
    registry: Registry,
    service_id: String,
    service_name: String,
    channel_total: GaugeVec,
    message_in_total: IntCounterVec,
    message_in_flow_bytes: IntCounterVec,
    message_out_flow_bytes: IntCounterVec,
    no_server_found: IntCounterVec,
    login_total: IntCounterVec,
    handler_duration: HistogramVec,
    talk_total: IntCounterVec,
    session_not_found: IntCounterVec,
    dispatch_fail_total: IntCounterVec,
    heartbeat_revoke_error_total: IntCounterVec,
    mailbox_full_total: IntCounterVec,
    send_to_ack: HistogramVec,
    royal_rpc: HistogramVec,
    royal_rpc_errors: IntCounterVec,
    pending_backlog: IntGaugeVec,
    pending_oldest_age: IntGaugeVec,
}

impl KimMetrics {
    pub fn new(service_id: &str, service_name: &str) -> Result<Arc<Self>, Error> {
        let registry = Registry::new();
        let labels = &["service_id", "service_name"];
        let channel_total = GaugeVec::new(Opts::new("kim_channel_total", "open channels"), labels)
            .map_err(|e| Error::Other(e.to_string()))?;
        let message_in_total = IntCounterVec::new(
            Opts::new("kim_message_in_total", "inbound messages"),
            labels,
        )
        .map_err(|e| Error::Other(e.to_string()))?;
        let message_in_flow_bytes = IntCounterVec::new(
            Opts::new("kim_message_in_flow_bytes", "inbound bytes"),
            labels,
        )
        .map_err(|e| Error::Other(e.to_string()))?;
        let message_out_flow_bytes = IntCounterVec::new(
            Opts::new("kim_message_out_flow_bytes", "outbound bytes"),
            labels,
        )
        .map_err(|e| Error::Other(e.to_string()))?;
        let no_server_found = IntCounterVec::new(
            Opts::new("kim_no_server_found_error_total", "forward with no adult"),
            labels,
        )
        .map_err(|e| Error::Other(e.to_string()))?;
        let login_total = IntCounterVec::new(
            Opts::new("kim_login_total", "login attempts"),
            &["service_id", "service_name", "status"],
        )
        .map_err(|e| Error::Other(e.to_string()))?;
        let handler_duration = HistogramVec::new(
            HistogramOpts::new("kim_handler_duration_seconds", "handler RT").buckets(vec![
                0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0,
            ]),
            &["service_id", "service_name", "command"],
        )
        .map_err(|e| Error::Other(e.to_string()))?;
        let talk_total = IntCounterVec::new(
            Opts::new("kim_talk_total", "talk commands"),
            &["service_id", "service_name", "kind"],
        )
        .map_err(|e| Error::Other(e.to_string()))?;
        let session_not_found = IntCounterVec::new(
            Opts::new("kim_session_not_found_total", "missing session"),
            labels,
        )
        .map_err(|e| Error::Other(e.to_string()))?;
        let dispatch_fail_total = IntCounterVec::new(
            Opts::new(
                "kim_dispatch_fail_total",
                "talk persist ok but online push did not complete",
            ),
            &["service_id", "service_name", "kind"],
        )
        .map_err(|e| Error::Other(e.to_string()))?;
        let heartbeat_revoke_error_total = IntCounterVec::new(
            Opts::new(
                "kim_heartbeat_revoke_error_total",
                "heartbeat revoke store/transport errors (bounded grace then disconnect)",
            ),
            labels,
        )
        .map_err(|e| Error::Other(e.to_string()))?;
        let mailbox_full_total = IntCounterVec::new(
            Opts::new(
                "kim_mailbox_full_total",
                "gateway downlink write mailbox full; slow connection disconnected",
            ),
            labels,
        )
        .map_err(|e| Error::Other(e.to_string()))?;
        let send_to_ack = HistogramVec::new(
            HistogramOpts::new(
                "kim_send_to_ack_seconds",
                "pending_delivery created_at to acked_at (held by the process that writes pending_delivery; Chat HTTP adapter must not observe)",
            )
            .buckets(vec![0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0]),
            labels,
        )
        .map_err(|e| Error::Other(e.to_string()))?;
        let royal_rpc = HistogramVec::new(
            HistogramOpts::new(
                "kim_royal_rpc_seconds",
                "Royal RPC end-to-end latency including retries",
            )
            .buckets(vec![
                0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0,
            ]),
            &["path_group"],
        )
        .map_err(|e| Error::Other(e.to_string()))?;
        let royal_rpc_errors = IntCounterVec::new(
            Opts::new(
                "kim_royal_rpc_errors_total",
                "Royal RPC final errors by path_group and cause",
            ),
            &["path_group", "cause"],
        )
        .map_err(|e| Error::Other(e.to_string()))?;
        let pending_backlog = IntGaugeVec::new(
            Opts::new(
                "kim_pending_delivery_backlog",
                "unacked pending_delivery rows",
            ),
            labels,
        )
        .map_err(|e| Error::Other(e.to_string()))?;
        let pending_oldest_age = IntGaugeVec::new(
            Opts::new(
                "kim_pending_delivery_oldest_age_seconds",
                "age of oldest unacked pending_delivery row",
            ),
            labels,
        )
        .map_err(|e| Error::Other(e.to_string()))?;

        registry
            .register(Box::new(channel_total.clone()))
            .map_err(|e| Error::Other(e.to_string()))?;
        registry
            .register(Box::new(message_in_total.clone()))
            .map_err(|e| Error::Other(e.to_string()))?;
        registry
            .register(Box::new(message_in_flow_bytes.clone()))
            .map_err(|e| Error::Other(e.to_string()))?;
        registry
            .register(Box::new(message_out_flow_bytes.clone()))
            .map_err(|e| Error::Other(e.to_string()))?;
        registry
            .register(Box::new(no_server_found.clone()))
            .map_err(|e| Error::Other(e.to_string()))?;
        registry
            .register(Box::new(login_total.clone()))
            .map_err(|e| Error::Other(e.to_string()))?;
        registry
            .register(Box::new(handler_duration.clone()))
            .map_err(|e| Error::Other(e.to_string()))?;
        registry
            .register(Box::new(talk_total.clone()))
            .map_err(|e| Error::Other(e.to_string()))?;
        registry
            .register(Box::new(session_not_found.clone()))
            .map_err(|e| Error::Other(e.to_string()))?;
        registry
            .register(Box::new(dispatch_fail_total.clone()))
            .map_err(|e| Error::Other(e.to_string()))?;
        registry
            .register(Box::new(heartbeat_revoke_error_total.clone()))
            .map_err(|e| Error::Other(e.to_string()))?;
        registry
            .register(Box::new(mailbox_full_total.clone()))
            .map_err(|e| Error::Other(e.to_string()))?;
        registry
            .register(Box::new(send_to_ack.clone()))
            .map_err(|e| Error::Other(e.to_string()))?;
        registry
            .register(Box::new(royal_rpc.clone()))
            .map_err(|e| Error::Other(e.to_string()))?;
        registry
            .register(Box::new(royal_rpc_errors.clone()))
            .map_err(|e| Error::Other(e.to_string()))?;
        registry
            .register(Box::new(pending_backlog.clone()))
            .map_err(|e| Error::Other(e.to_string()))?;
        registry
            .register(Box::new(pending_oldest_age.clone()))
            .map_err(|e| Error::Other(e.to_string()))?;

        Ok(Arc::new(Self {
            registry,
            service_id: service_id.into(),
            service_name: service_name.into(),
            channel_total,
            message_in_total,
            message_in_flow_bytes,
            message_out_flow_bytes,
            no_server_found,
            login_total,
            handler_duration,
            talk_total,
            session_not_found,
            dispatch_fail_total,
            heartbeat_revoke_error_total,
            mailbox_full_total,
            send_to_ack,
            royal_rpc,
            royal_rpc_errors,
            pending_backlog,
            pending_oldest_age,
        }))
    }

    pub fn registry(&self) -> Registry {
        self.registry.clone()
    }

    pub fn scrape_text(&self) -> Result<String, Error> {
        let encoder = TextEncoder::new();
        let families = self.registry.gather();
        let mut buf = Vec::new();
        encoder
            .encode(&families, &mut buf)
            .map_err(|e| Error::Other(e.to_string()))?;
        String::from_utf8(buf).map_err(|e| Error::Other(e.to_string()))
    }

    fn svc(&self) -> [&str; 2] {
        [self.service_id.as_str(), self.service_name.as_str()]
    }

    pub fn on_channel_open(&self) {
        self.channel_total.with_label_values(&self.svc()).inc();
    }

    pub fn on_channel_close(&self) {
        self.channel_total.with_label_values(&self.svc()).dec();
    }

    pub fn on_message_in(&self, nbytes: u64) {
        self.message_in_total.with_label_values(&self.svc()).inc();
        self.message_in_flow_bytes
            .with_label_values(&self.svc())
            .inc_by(nbytes);
    }

    pub fn on_message_out(&self, nbytes: u64) {
        self.message_out_flow_bytes
            .with_label_values(&self.svc())
            .inc_by(nbytes);
    }

    pub fn on_no_server(&self) {
        self.no_server_found.with_label_values(&self.svc()).inc();
    }

    pub fn on_login(&self, status: i32) {
        self.login_total
            .with_label_values(&[
                self.service_id.as_str(),
                self.service_name.as_str(),
                &status.to_string(),
            ])
            .inc();
    }

    /// Held by the process that writes `pending_delivery` (production: Royal).
    /// Chat's HTTP adapter must not call this.
    pub fn observe_send_to_ack(&self, dt: Duration) {
        self.send_to_ack
            .with_label_values(&self.svc())
            .observe(dt.as_secs_f64());
    }

    pub fn observe_royal_rpc(&self, path_group: &str, dt: Duration) {
        self.royal_rpc
            .with_label_values(&[path_group])
            .observe(dt.as_secs_f64());
    }

    pub fn on_royal_rpc_error(&self, path_group: &str, cause: &str) {
        self.royal_rpc_errors
            .with_label_values(&[path_group, cause])
            .inc();
    }

    pub fn set_pending_backlog(&self, count: i64, oldest_age: i64) {
        self.pending_backlog
            .with_label_values(&self.svc())
            .set(count);
        self.pending_oldest_age
            .with_label_values(&self.svc())
            .set(oldest_age);
    }

    pub fn observe_handler(&self, command: &str, dt: Duration) {
        let cmd = if COMMANDS.contains(&command) {
            command
        } else {
            "other"
        };
        self.handler_duration
            .with_label_values(&[self.service_id.as_str(), self.service_name.as_str(), cmd])
            .observe(dt.as_secs_f64());
    }

    pub fn on_talk(&self, kind: &str) {
        self.talk_total
            .with_label_values(&[self.service_id.as_str(), self.service_name.as_str(), kind])
            .inc();
    }

    pub fn on_session_not_found(&self) {
        self.session_not_found.with_label_values(&self.svc()).inc();
    }

    pub fn on_dispatch_fail(&self, kind: &str) {
        self.dispatch_fail_total
            .with_label_values(&[self.service_id.as_str(), self.service_name.as_str(), kind])
            .inc();
    }

    pub fn on_heartbeat_revoke_error(&self) {
        self.heartbeat_revoke_error_total
            .with_label_values(&self.svc())
            .inc();
    }

    pub fn on_mailbox_full(&self) {
        self.mailbox_full_total.with_label_values(&self.svc()).inc();
    }
}

/// Mergeable axum router: GET /metrics, GET /health.
pub fn router(registry: Registry) -> Router {
    Router::new()
        .route("/metrics", get(metrics_handler))
        .route("/health", get(health_handler))
        .with_state(registry)
}

async fn health_handler() -> &'static str {
    "ok"
}

async fn metrics_handler(State(reg): State<Registry>) -> Result<String, StatusCode> {
    let encoder = TextEncoder::new();
    let families = reg.gather();
    let mut buf = Vec::new();
    encoder
        .encode(&families, &mut buf)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    String::from_utf8(buf).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn serve(listen: SocketAddr, registry: Registry) -> Result<(), std::io::Error> {
    let listener = tokio::net::TcpListener::bind(listen).await?;
    serve_listener(listener, registry).await
}

pub async fn serve_listener(
    listener: tokio::net::TcpListener,
    registry: Registry,
) -> Result<(), std::io::Error> {
    let app = router(registry);
    axum::serve(listener, app).await
}

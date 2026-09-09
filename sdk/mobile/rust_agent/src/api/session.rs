//! Thin FFI over `kim-agent-host` (Goose). Isolated from `kim_client_ffi`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use kim_agent_host::{AgentHost, HostError, HostEvent, ProviderConfig, ProviderKind};
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio_util::sync::CancellationToken;

use super::rt;
use crate::frb_generated::StreamSink;

pub struct SessionOpenOpts {
    pub model: String,
    pub llm_backend: String,
    pub resume_on_open: bool,
    pub base_url: String,
    pub api_key: String,
    pub enable_fs_tools: bool,
    pub bash_enabled: bool,
}

impl Default for SessionOpenOpts {
    fn default() -> Self {
        Self {
            model: "gpt-4o".into(),
            llm_backend: "openai".into(),
            resume_on_open: true,
            base_url: "https://api.openai.com/v1".into(),
            api_key: String::new(),
            enable_fs_tools: false,
            bash_enabled: false,
        }
    }
}

#[derive(Clone)]
pub struct AgentUiEvent {
    pub kind: String,
    pub operation_id: String,
    pub call_id: String,
    pub name: String,
    pub delta: String,
    pub arguments_json: String,
    pub output_preview: String,
    pub ok: bool,
    pub stop_reason: String,
    pub message: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub resumed_ops: Vec<String>,
}

impl AgentUiEvent {
    fn base(kind: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            operation_id: String::new(),
            call_id: String::new(),
            name: String::new(),
            delta: String::new(),
            arguments_json: String::new(),
            output_preview: String::new(),
            ok: false,
            stop_reason: String::new(),
            message: String::new(),
            input_tokens: 0,
            output_tokens: 0,
            resumed_ops: Vec::new(),
        }
    }

    fn session_ready() -> Self {
        Self::base("session_ready")
    }

    fn operation_started(operation_id: String) -> Self {
        let mut e = Self::base("operation_started");
        e.operation_id = operation_id;
        e
    }

    fn text_delta(operation_id: String, delta: String) -> Self {
        let mut e = Self::base("assistant_text_delta");
        e.operation_id = operation_id;
        e.delta = delta;
        e
    }

    fn completed(operation_id: String, text: String) -> Self {
        let mut e = Self::base("assistant_finished");
        e.operation_id = operation_id;
        e.message = text;
        e.stop_reason = "completed".into();
        e.ok = true;
        e
    }

    fn failed(operation_id: String, message: String) -> Self {
        let mut e = Self::base("failed");
        e.operation_id = operation_id;
        e.message = message;
        e
    }

    fn aborted(operation_id: String) -> Self {
        let mut e = Self::base("aborted");
        e.operation_id = operation_id;
        e.stop_reason = "aborted".into();
        e
    }
}

pub struct ResumeReportDto {
    pub resumed_ops: Vec<String>,
    pub statuses: Vec<String>,
}

pub struct SessionSnapshotDto {
    pub busy: bool,
    pub last_operation_id: String,
}

struct Shared {
    host: RwLock<AgentHost>,
    session_id: String,
    events: broadcast::Sender<AgentUiEvent>,
    busy: AtomicBool,
    cancel: Mutex<Option<CancellationToken>>,
}

pub struct AgentSession {
    inner: Arc<Shared>,
}

fn config_from_opts(opts: &SessionOpenOpts) -> Result<ProviderConfig, String> {
    let kind = ProviderKind::parse(&opts.llm_backend).map_err(|e| e.to_string())?;
    Ok(ProviderConfig {
        kind,
        base_url: opts.base_url.clone(),
        api_key: opts.api_key.clone(),
        model: opts.model.clone(),
    })
}

pub fn session_open(
    sqlite_path: String,
    _project_root: String,
    opts: SessionOpenOpts,
) -> Result<AgentSession, String> {
    let _guard = rt().enter();
    let session_id = if sqlite_path.trim().is_empty() {
        uuid::Uuid::new_v4().to_string()
    } else {
        sqlite_path
    };
    let host = AgentHost::new(config_from_opts(&opts)?).map_err(map_host_err)?;
    let (tx, _) = broadcast::channel(256);
    let _ = tx.send(AgentUiEvent::session_ready());
    Ok(AgentSession {
        inner: Arc::new(Shared {
            host: RwLock::new(host),
            session_id,
            events: tx,
            busy: AtomicBool::new(false),
            cancel: Mutex::new(None),
        }),
    })
}

impl AgentSession {
    pub fn prompt(&self, text: String) -> Result<String, String> {
        if self.inner.busy.swap(true, Ordering::SeqCst) {
            return Err("agent busy".into());
        }
        let text = text.trim().to_string();
        if text.is_empty() {
            self.inner.busy.store(false, Ordering::SeqCst);
            return Err("empty prompt".into());
        }

        let inner = self.inner.clone();
        let (op_tx, op_rx) = std::sync::mpsc::channel();
        let cancel = CancellationToken::new();
        if let Ok(mut g) = inner.cancel.lock() {
            *g = Some(cancel.clone());
        }

        rt().spawn(async move {
            let op = uuid::Uuid::new_v4().to_string();
            let _ = inner
                .events
                .send(AgentUiEvent::operation_started(op.clone()));
            let _ = op_tx.send(Ok(op.clone()));

            let (tx, mut rx) = mpsc::channel(64);
            let events = inner.events.clone();
            let op_for_pump = op.clone();
            let pump = tokio::spawn(async move {
                while let Some(ev) = rx.recv().await {
                    match ev {
                        HostEvent::TextDelta { delta } => {
                            let _ =
                                events.send(AgentUiEvent::text_delta(op_for_pump.clone(), delta));
                        }
                        HostEvent::Finished { text } => {
                            let _ = events.send(AgentUiEvent::completed(op_for_pump.clone(), text));
                        }
                        HostEvent::Failed { message } => {
                            let _ = events.send(AgentUiEvent::failed(op_for_pump.clone(), message));
                        }
                    }
                }
            });

            let host = inner.host.read().await;
            let result = host.prompt(&inner.session_id, &text, tx, cancel).await;
            drop(host);
            let _ = pump.await;

            match result {
                Ok(reply) => {
                    let _ = inner.events.send(AgentUiEvent::completed(op, reply));
                }
                Err(HostError::Failed(msg)) if msg.contains("cancel") => {
                    let _ = inner.events.send(AgentUiEvent::aborted(op));
                }
                Err(err) => {
                    let _ = inner
                        .events
                        .send(AgentUiEvent::failed(op, map_host_err(err)));
                }
            }
            inner.busy.store(false, Ordering::SeqCst);
        });

        op_rx
            .recv_timeout(Duration::from_secs(30))
            .map_err(|_| "prompt start timeout".to_string())?
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn listen(&self, sink: StreamSink<AgentUiEvent>) -> Result<(), String> {
        let _guard = rt().enter();
        let mut rx = self.inner.events.subscribe();
        rt().spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(ev) => {
                        if sink.add(ev).is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });
        Ok(())
    }

    pub fn abort(&self) -> Result<(), String> {
        if let Ok(g) = self.inner.cancel.lock() {
            if let Some(token) = g.as_ref() {
                token.cancel();
            }
        }
        Ok(())
    }

    pub fn resume(&self) -> Result<ResumeReportDto, String> {
        Ok(ResumeReportDto {
            resumed_ops: Vec::new(),
            statuses: Vec::new(),
        })
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn snapshot(&self) -> Result<SessionSnapshotDto, String> {
        Ok(SessionSnapshotDto {
            busy: self.inner.busy.load(Ordering::SeqCst),
            last_operation_id: String::new(),
        })
    }

    pub fn reconfigure(&self, opts: SessionOpenOpts) -> Result<(), String> {
        let _guard = rt().enter();
        let host = AgentHost::new(config_from_opts(&opts)?).map_err(map_host_err)?;
        let inner = self.inner.clone();
        rt().block_on(async move {
            *inner.host.write().await = host;
        });
        Ok(())
    }

    pub fn close(&self) -> Result<(), String> {
        let _ = self.abort();
        Ok(())
    }
}

fn map_host_err(err: HostError) -> String {
    err.to_string()
}

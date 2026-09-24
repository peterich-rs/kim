//! Append-only file sink for mobile `tracing` events.
//!
//! Call [`init_beside`] once from each FFI library. `kim_client_ffi` and
//! `kim_agent_ffi` each link their own copy of `tracing`, so each needs its
//! own subscriber. Libraries must keep emitting events and must not call this.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, Once};
use std::time::SystemTime;

use tracing::field::{Field, Visit};
use tracing::Subscriber;
use tracing_subscriber::layer::Context;
use tracing_subscriber::prelude::*;
use tracing_subscriber::Layer;

/// Open `{parent(anchor)}/{file_name}`, creating the directory if needed.
/// The first successful call installs the process subscriber. Later calls
/// keep that file.
pub fn init_beside(anchor: &str, file_name: &str) {
    let anchor = Path::new(anchor);
    let dir = anchor
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(std::env::temp_dir);
    init(dir.join(file_name));
}

fn init(path: PathBuf) {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
        let mut opts = OpenOptions::new();
        opts.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let Ok(file) = opts.open(&path) else {
            return;
        };
        let layer = FileLayer {
            file: Mutex::new(file),
        };
        let kim_info = tracing_subscriber::filter::filter_fn(|meta| {
            meta.target().starts_with("kim") && *meta.level() <= tracing::Level::INFO
        });
        let _ = tracing_subscriber::registry()
            .with(layer.with_filter(kim_info))
            .try_init();
    });
}

struct FileLayer {
    file: Mutex<std::fs::File>,
}

impl<S> Layer<S> for FileLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        let mut fields = FieldText::default();
        event.record(&mut fields);
        let loc = match (meta.module_path(), meta.line()) {
            (Some(module), Some(line)) => format!("{module}:{line}"),
            (Some(module), None) => module.to_string(),
            (None, Some(line)) => format!("?:{line}"),
            (None, None) => "-".to_string(),
        };
        let mut msg = fields.message;
        if !fields.rest.is_empty() {
            if !msg.is_empty() {
                msg.push(' ');
            }
            msg.push_str(&fields.rest);
        }
        // Double spaces between columns so level / time / loc / msg scan cleanly.
        let line = format!(
            "[{}]  {}  {}  {}\n",
            meta.level().as_str().to_ascii_lowercase(),
            humantime::format_rfc3339_millis(SystemTime::now()),
            loc,
            msg.replace(['\n', '\r'], " ")
        );
        let mut file = self.file.lock().unwrap_or_else(|err| err.into_inner());
        let _ = file.write_all(line.as_bytes());
        let _ = file.flush();
    }
}

#[derive(Default)]
struct FieldText {
    message: String,
    rest: String,
}

impl FieldText {
    fn push_field(&mut self, name: &str, value: &str) {
        if name == "message" {
            self.message = value.to_string();
            return;
        }
        if !self.rest.is_empty() {
            self.rest.push(' ');
        }
        self.rest.push_str(name);
        self.rest.push('=');
        self.rest.push_str(&value.replace(['\n', '\r'], " "));
    }
}

impl Visit for FieldText {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let rendered = format!("{value:?}");
        let trimmed = rendered.trim_matches('"');
        self.push_field(field.name(), trimmed);
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.push_field(field.name(), value);
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.push_field(field.name(), &value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.push_field(field.name(), &value.to_string());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.push_field(field.name(), if value { "true" } else { "false" });
    }
}

#[cfg(test)]
mod tests {
    use super::init_beside;

    #[test]
    fn info_line_uses_level_time_module_and_line() {
        let dir = std::env::temp_dir().join(format!("kim-log-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let anchor = dir.join("kim-cache.db");
        init_beside(&anchor.to_string_lossy(), "kim.log");
        tracing::info!(account = "alice", "session start");
        let text = std::fs::read_to_string(dir.join("kim.log")).expect("log");
        let line = text.lines().next().expect("line");
        let mut parts = line.splitn(4, "  ");
        let level = parts.next().expect("level");
        let ts = parts.next().expect("ts");
        let loc = parts.next().expect("loc");
        let msg = parts.next().expect("msg");
        assert_eq!(level, "[info]");
        assert!(ts.contains('T'), "{ts}");
        assert!(loc.starts_with("kim_log::tests:"), "{loc}");
        assert!(!loc.contains('/'), "{loc}");
        assert!(msg.contains("session start"), "{msg}");
        assert!(msg.contains("account=alice"), "{msg}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

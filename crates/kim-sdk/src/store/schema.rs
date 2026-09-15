pub const SCHEMA_VERSION: i64 = 6;
pub const MAX_MESSAGES: i32 = 400;

pub const CREATE_META: &str = r"
CREATE TABLE IF NOT EXISTS meta (
  key TEXT PRIMARY KEY NOT NULL,
  value TEXT NOT NULL
)
";

pub const CREATE_THREADS: &str = r"
CREATE TABLE IF NOT EXISTS threads (
  account TEXT NOT NULL,
  id TEXT NOT NULL,
  kind TEXT NOT NULL,
  title TEXT NOT NULL,
  last_body TEXT NOT NULL DEFAULT '',
  last_at INTEGER NOT NULL DEFAULT 0,
  unread INTEGER NOT NULL DEFAULT 0,
  avatar TEXT NOT NULL DEFAULT '',
  PRIMARY KEY (account, id)
)
";

pub const CREATE_MESSAGES: &str = r"
CREATE TABLE IF NOT EXISTS messages (
  account TEXT NOT NULL,
  dest TEXT NOT NULL,
  key TEXT NOT NULL,
  sender TEXT NOT NULL,
  body TEXT NOT NULL,
  at INTEGER NOT NULL DEFAULT 0,
  sys INTEGER NOT NULL DEFAULT 0,
  failed INTEGER NOT NULL DEFAULT 0,
  kind TEXT NOT NULL DEFAULT 'text',
  width INTEGER NOT NULL DEFAULT 0,
  height INTEGER NOT NULL DEFAULT 0,
  message_id INTEGER NOT NULL DEFAULT 0,
  batch_id TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'sent',
  local_path TEXT NOT NULL DEFAULT '',
  PRIMARY KEY (account, dest, key)
)
";

pub const CREATE_OUTBOX: &str = r"
CREATE TABLE IF NOT EXISTS outbox (
  account TEXT NOT NULL,
  client_id TEXT NOT NULL,
  dest TEXT NOT NULL,
  kind INTEGER NOT NULL,
  payload_type INTEGER NOT NULL,
  body TEXT NOT NULL,
  extra TEXT NOT NULL DEFAULT '',
  local_path TEXT NOT NULL DEFAULT '',
  mime TEXT NOT NULL DEFAULT '',
  width INTEGER NOT NULL DEFAULT 0,
  height INTEGER NOT NULL DEFAULT 0,
  byte_size INTEGER NOT NULL DEFAULT 0,
  batch_id TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL,
  attempt INTEGER NOT NULL DEFAULT 0,
  next_attempt_at INTEGER NOT NULL DEFAULT 0,
  message_id INTEGER NOT NULL DEFAULT 0,
  last_error TEXT NOT NULL DEFAULT '',
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (account, client_id)
)
";

pub const CREATE_SYNC_CURSORS: &str = r"
CREATE TABLE IF NOT EXISTS sync_cursors (
  account TEXT NOT NULL,
  name TEXT NOT NULL,
  cursor INTEGER NOT NULL DEFAULT 0,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (account, name)
)
";

pub const CREATE_READ_WATERMARKS: &str = r"
CREATE TABLE IF NOT EXISTS read_watermarks (
  account TEXT NOT NULL,
  dest TEXT NOT NULL,
  last_read_message_id INTEGER NOT NULL DEFAULT 0,
  last_read_at INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (account, dest)
)
";

pub const CREATE_TIMELINE_META: &str = r"
CREATE TABLE IF NOT EXISTS timeline_meta (
  account TEXT NOT NULL,
  dest TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (account, dest)
)
";

pub const IDX_MESSAGES_BY_ID: &str =
    "CREATE INDEX IF NOT EXISTS messages_by_id ON messages (account, dest, message_id)";
pub const IDX_MESSAGES_MID_UNIQUE: &str = "CREATE UNIQUE INDEX IF NOT EXISTS messages_mid_unique \
     ON messages (account, dest, message_id) WHERE message_id != 0";
pub const IDX_MESSAGES_BY_THREAD_KEY: &str = "CREATE INDEX IF NOT EXISTS messages_by_thread_key \
     ON messages (account, dest, at DESC, key DESC)";
pub const IDX_MESSAGES_PENDING: &str =
    "CREATE INDEX IF NOT EXISTS messages_pending ON messages (account, status, at, key)";
pub const IDX_OUTBOX_DUE: &str =
    "CREATE INDEX IF NOT EXISTS outbox_due ON outbox (account, status, next_attempt_at)";

pub const CREATE_CONTACTS: &str = r"
CREATE TABLE IF NOT EXISTS contacts (
  account TEXT NOT NULL,
  peer TEXT NOT NULL,
  relation TEXT NOT NULL,
  nickname TEXT NOT NULL DEFAULT '',
  avatar TEXT NOT NULL DEFAULT '',
  bio TEXT NOT NULL DEFAULT '',
  kind INTEGER NOT NULL DEFAULT 1,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (account, peer)
)
";

pub const CREATE_SETTINGS: &str = r"
CREATE TABLE IF NOT EXISTS settings (
  account TEXT NOT NULL PRIMARY KEY,
  ws_url TEXT NOT NULL DEFAULT '',
  http_origin TEXT NOT NULL DEFAULT '',
  env TEXT NOT NULL DEFAULT 'prod',
  locale TEXT NOT NULL DEFAULT '',
  agent_flags TEXT NOT NULL DEFAULT '{}'
)
";

pub const CREATE_AGENT_PROFILES: &str = r"
CREATE TABLE IF NOT EXISTS agent_profiles (
  account TEXT NOT NULL,
  profile_id TEXT NOT NULL,
  nickname TEXT NOT NULL,
  server_account TEXT NOT NULL DEFAULT '',
  body_json TEXT NOT NULL,
  body_blob BLOB,
  placement TEXT NOT NULL DEFAULT 'local',
  key_ciphertext BLOB,
  updated_at INTEGER NOT NULL,
  deleted_at INTEGER,
  PRIMARY KEY (account, profile_id)
)
";

pub const CREATE_PROVIDER_ACCOUNTS: &str = r"
CREATE TABLE IF NOT EXISTS provider_accounts (
  account TEXT NOT NULL,
  id TEXT NOT NULL,
  vendor_id TEXT NOT NULL,
  base_url TEXT NOT NULL DEFAULT '',
  key_ref TEXT NOT NULL DEFAULT '',
  display_name TEXT NOT NULL DEFAULT '',
  models_json TEXT NOT NULL DEFAULT '[]',
  key_ciphertext BLOB,
  updated_at INTEGER NOT NULL,
  deleted_at INTEGER,
  PRIMARY KEY (account, id)
)
";

pub const CREATE_AGENT_DEVICE_OVERLAY: &str = r"
CREATE TABLE IF NOT EXISTS agent_device_overlay (
  account TEXT NOT NULL,
  profile_id TEXT NOT NULL,
  workspace_path TEXT NOT NULL DEFAULT '',
  workspace_bookmark TEXT NOT NULL DEFAULT '',
  user_agents_skills TEXT NOT NULL DEFAULT '',
  PRIMARY KEY (account, profile_id)
)
";

pub const CREATE_AGENT_PERMISSIONS: &str = r"
CREATE TABLE IF NOT EXISTS agent_permissions (
  account TEXT NOT NULL,
  profile_id TEXT NOT NULL,
  tool TEXT NOT NULL,
  decision TEXT NOT NULL,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (account, profile_id, tool)
)
";

pub const CREATE_MEDIA_CACHE: &str = r"
CREATE TABLE IF NOT EXISTS media_cache (
  url TEXT PRIMARY KEY NOT NULL,
  local_path TEXT NOT NULL,
  byte_size INTEGER NOT NULL,
  last_access INTEGER NOT NULL
)
";

pub const MEDIA_CACHE_CAP_BYTES: i64 = 512 * 1024 * 1024;
pub const SEARCH_CAP: i32 = 50;

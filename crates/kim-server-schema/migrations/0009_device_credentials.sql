CREATE TABLE device_credentials (
    device_id TEXT PRIMARY KEY,
    app TEXT NOT NULL,
    account TEXT NOT NULL,
    secret_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    revoked_at TIMESTAMPTZ
);

CREATE INDEX device_credentials_account
    ON device_credentials (app, account);

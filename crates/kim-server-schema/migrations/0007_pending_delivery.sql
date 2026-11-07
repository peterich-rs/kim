CREATE TABLE pending_delivery (
    app TEXT NOT NULL,
    account TEXT NOT NULL,
    target_id TEXT NOT NULL,
    message_id BIGINT NOT NULL REFERENCES message_content (id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL,
    acked_at TIMESTAMPTZ,
    PRIMARY KEY (app, account, target_id, message_id)
);

CREATE INDEX pending_delivery_pull
    ON pending_delivery (app, account, target_id, created_at, message_id)
    WHERE acked_at IS NULL;

CREATE INDEX pending_delivery_expires
    ON pending_delivery (expires_at);

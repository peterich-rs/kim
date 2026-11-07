CREATE TABLE message_idempotency (
    app TEXT NOT NULL,
    sender TEXT NOT NULL,
    client_id TEXT NOT NULL,
    message_id BIGINT NOT NULL REFERENCES message_content (id),
    send_time BIGINT NOT NULL,
    PRIMARY KEY (app, sender, client_id)
);

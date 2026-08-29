CREATE TABLE message_content (
    id BIGINT PRIMARY KEY,
    app TEXT NOT NULL,
    msg_type SMALLINT NOT NULL,
    body TEXT NOT NULL,
    extra TEXT NOT NULL DEFAULT '',
    send_time BIGINT NOT NULL
);

CREATE TABLE message_index (
    id BIGINT PRIMARY KEY,
    app TEXT NOT NULL,
    account_a TEXT NOT NULL,
    account_b TEXT NOT NULL,
    direction SMALLINT NOT NULL CHECK (direction IN (0, 1)),
    message_id BIGINT NOT NULL REFERENCES message_content (id),
    group_id TEXT NOT NULL DEFAULT '',
    send_time BIGINT NOT NULL
);

CREATE INDEX message_index_inbox
    ON message_index (app, account_a, direction, send_time);

CREATE INDEX message_index_message_id
    ON message_index (message_id);

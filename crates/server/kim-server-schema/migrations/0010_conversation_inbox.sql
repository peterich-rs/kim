CREATE TABLE conversation_inbox (
    app             TEXT NOT NULL,
    account         TEXT NOT NULL,
    dest            TEXT NOT NULL,
    kind            SMALLINT NOT NULL CHECK (kind IN (0, 1)),
    last_message_id BIGINT NOT NULL REFERENCES message_content (id),
    last_send_time  BIGINT NOT NULL,
    last_sender     TEXT NOT NULL,
    last_body       TEXT NOT NULL,
    last_msg_type   SMALLINT NOT NULL,
    unread          INT NOT NULL DEFAULT 0,
    PRIMARY KEY (app, account, dest, kind)
);

CREATE INDEX conversation_inbox_recent
    ON conversation_inbox (app, account, last_send_time DESC);

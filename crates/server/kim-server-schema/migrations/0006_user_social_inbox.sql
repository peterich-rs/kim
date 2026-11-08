ALTER TABLE users ADD COLUMN nickname TEXT NOT NULL DEFAULT '';
ALTER TABLE users ADD COLUMN avatar TEXT NOT NULL DEFAULT '';
ALTER TABLE users ADD COLUMN bio TEXT NOT NULL DEFAULT '';

UPDATE users SET nickname = account WHERE nickname = '';

CREATE INDEX users_nickname_prefix ON users (app, lower(nickname) text_pattern_ops);

CREATE TABLE friend_requests (
    app TEXT NOT NULL,
    from_account TEXT NOT NULL,
    to_account TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (app, from_account, to_account),
    CHECK (from_account <> to_account)
);

CREATE INDEX friend_requests_inbox
    ON friend_requests (app, to_account, created_at DESC);

CREATE TABLE friendships (
    app TEXT NOT NULL,
    account_a TEXT NOT NULL,
    account_b TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (app, account_a, account_b),
    CHECK (account_a < account_b)
);

CREATE TABLE blocks (
    app TEXT NOT NULL,
    account TEXT NOT NULL,
    blocked TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (app, account, blocked),
    CHECK (account <> blocked)
);

CREATE INDEX blocks_blocked ON blocks (app, blocked);

CREATE TABLE conversation_reads (
    app TEXT NOT NULL,
    account TEXT NOT NULL,
    peer TEXT NOT NULL DEFAULT '',
    group_id TEXT NOT NULL DEFAULT '',
    last_read_id BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (app, account, peer, group_id),
    CHECK (
        (peer <> '' AND group_id = '')
        OR (peer = '' AND group_id <> '')
    )
);

CREATE INDEX message_index_thread
    ON message_index (app, account_a, group_id, account_b, message_id DESC);

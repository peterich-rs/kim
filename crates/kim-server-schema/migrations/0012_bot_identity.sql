ALTER TABLE users ADD COLUMN client_profile_id TEXT NOT NULL DEFAULT '';

CREATE UNIQUE INDEX users_bot_owner_profile
    ON users (app, owner_account, client_profile_id)
    WHERE kind = 'bot' AND client_profile_id <> '';

CREATE INDEX users_bot_owner
    ON users (app, owner_account)
    WHERE kind = 'bot';

CREATE TABLE bot_turns (
    app TEXT NOT NULL,
    bot_account TEXT NOT NULL,
    in_reply_to BIGINT NOT NULL,
    reply_message_id BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (app, bot_account, in_reply_to)
);

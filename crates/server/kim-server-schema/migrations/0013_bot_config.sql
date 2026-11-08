-- Owner-only Goose/runtime projection. Never store API keys / keyRef / secret URLs.
CREATE TABLE bot_config (
    app TEXT NOT NULL,
    bot_account TEXT NOT NULL,
    model TEXT NOT NULL DEFAULT '',
    thinking_effort TEXT NOT NULL DEFAULT '',
    context_tokens INTEGER,
    visibility TEXT NOT NULL DEFAULT 'private',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (app, bot_account),
    FOREIGN KEY (app, bot_account) REFERENCES users (app, account) ON DELETE CASCADE,
    CONSTRAINT bot_config_visibility_chk CHECK (visibility IN ('private', 'owner_card', 'public'))
);

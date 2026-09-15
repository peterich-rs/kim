-- Owner-only AgentSpec copy. spec BYTEA is opaque kim.agent.AgentSpec bytes.
CREATE TABLE agent_specs (
    app TEXT NOT NULL,
    owner TEXT NOT NULL,
    profile_id TEXT NOT NULL,
    nickname TEXT NOT NULL DEFAULT '',
    server_account TEXT NOT NULL DEFAULT '',
    spec BYTEA NOT NULL,
    key_ciphertext BYTEA,
    updated_at BIGINT NOT NULL,
    deleted_at BIGINT,
    PRIMARY KEY (app, owner, profile_id)
);

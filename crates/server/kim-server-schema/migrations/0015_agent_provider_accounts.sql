-- Owner-only ProviderAccount copy. No API key bytes (A-KD 13 / A-KD 8).
CREATE TABLE agent_provider_accounts (
    app TEXT NOT NULL,
    owner TEXT NOT NULL,
    id TEXT NOT NULL,
    vendor_id TEXT NOT NULL,
    base_url TEXT NOT NULL DEFAULT '',
    key_ref TEXT NOT NULL DEFAULT '',
    display_name TEXT NOT NULL DEFAULT '',
    models TEXT NOT NULL DEFAULT '[]',
    updated_at BIGINT NOT NULL,
    deleted_at BIGINT,
    PRIMARY KEY (app, owner, id)
);

-- Bot identity deletion receipts. The message store proves cleanup scope; Royal only records it.
CREATE TABLE IF NOT EXISTS bot_deletion (
    deletion_id TEXT PRIMARY KEY,
    app TEXT NOT NULL,
    bot_account TEXT NOT NULL,
    original_owner TEXT NOT NULL,
    cleanup_scope TEXT NOT NULL,
    status TEXT NOT NULL,
    UNIQUE (app, bot_account)
);

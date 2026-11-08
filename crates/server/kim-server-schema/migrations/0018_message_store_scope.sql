SET LOCAL lock_timeout = '5s';
SET LOCAL statement_timeout = '30s';

-- One persistent identity per message dataset, shared by every pool/process.
-- A separately writable database clone must receive a fresh dataset_id.
CREATE TABLE message_store_identity (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    dataset_id UUID NOT NULL UNIQUE DEFAULT gen_random_uuid()
);
INSERT INTO message_store_identity (singleton) VALUES (TRUE);

-- A completed cleanup also fences late in-flight writes to this deleted bot DM.
-- Generic purge-dm does not create a fence; clearing a normal thread stays reusable.
CREATE TABLE message_thread_tombstone (
    app TEXT NOT NULL,
    account_a TEXT NOT NULL,
    account_b TEXT NOT NULL,
    PRIMARY KEY (app, account_a, account_b),
    CHECK (account_a COLLATE "C" < account_b COLLATE "C")
);

ALTER TABLE bot_deletion ADD CONSTRAINT bot_deletion_status_check
    CHECK (status IN ('pending', 'done'));

-- not auto-applied. sqlx::migrate!("./migrations") ignores this file.
-- Optional HASH partition sketch for message_index at large scale.

-- Example (do not run from default Demo):
-- ALTER TABLE message_index PARTITION BY HASH (account) PARTITIONS 16;

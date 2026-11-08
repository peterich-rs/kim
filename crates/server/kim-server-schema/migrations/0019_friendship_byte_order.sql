SET LOCAL lock_timeout = '5s';
SET LOCAL statement_timeout = '30s';

-- Match social::ordered_pair even when the database locale sorts account IDs
-- differently. Keep historical migration checksums and existing rows intact.
ALTER TABLE friendships DROP CONSTRAINT friendships_check;
ALTER TABLE friendships ADD CONSTRAINT friendships_check
    CHECK (account_a COLLATE "C" < account_b COLLATE "C") NOT VALID;
ALTER TABLE friendships VALIDATE CONSTRAINT friendships_check;

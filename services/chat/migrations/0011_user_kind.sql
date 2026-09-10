ALTER TABLE users ADD COLUMN kind TEXT NOT NULL DEFAULT 'user';
ALTER TABLE users ADD COLUMN owner_account TEXT NOT NULL DEFAULT '';
ALTER TABLE users ADD CONSTRAINT users_kind_check CHECK (kind IN ('user', 'bot'));

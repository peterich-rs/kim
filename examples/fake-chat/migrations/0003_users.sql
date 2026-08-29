CREATE TABLE users (
    app TEXT NOT NULL,
    account TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (app, account)
);

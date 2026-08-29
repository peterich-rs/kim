CREATE TABLE groups (
    app TEXT NOT NULL,
    id TEXT NOT NULL,
    name TEXT NOT NULL,
    avatar TEXT NOT NULL DEFAULT '',
    introduction TEXT NOT NULL DEFAULT '',
    owner TEXT NOT NULL,
    PRIMARY KEY (app, id)
);

CREATE TABLE group_members (
    app TEXT NOT NULL,
    group_id TEXT NOT NULL,
    account TEXT NOT NULL,
    pos INT NOT NULL,
    PRIMARY KEY (app, group_id, account)
);

CREATE INDEX group_members_list
    ON group_members (app, group_id, pos);

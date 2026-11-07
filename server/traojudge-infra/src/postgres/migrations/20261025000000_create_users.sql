CREATE TABLE users (
    id TEXT PRIMARY KEY CHECK (id ~ '^[A-Za-z0-9_-]{1,128}$'),
    display_name TEXT NOT NULL CHECK (char_length(display_name) BETWEEN 1 AND 80),
    bio TEXT NOT NULL DEFAULT '' CHECK (char_length(bio) <= 4000),
    github_id TEXT NOT NULL DEFAULT '',
    atcoder_id TEXT NOT NULL DEFAULT '',
    x_id TEXT NOT NULL DEFAULT '',
    revision TEXT NOT NULL CHECK (char_length(revision) BETWEEN 1 AND 256)
);

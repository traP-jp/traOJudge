-- 既存の作成者不明の問題はNULLのまま残す。推測した利用者を作成者にしない。
ALTER TABLE problems ADD COLUMN created_by_user_id UUID REFERENCES users(id);

CREATE TABLE contests (
    id UUID PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('regular','virtual')),
    created_by_user_id UUID NOT NULL REFERENCES users(id),
    title TEXT NOT NULL CHECK (char_length(title) BETWEEN 1 AND 200),
    description TEXT NOT NULL DEFAULT '' CHECK (char_length(description) <= 200000),
    visibility TEXT NOT NULL CHECK (visibility IN ('public','private')),
    participation_mode TEXT NOT NULL CHECK (participation_mode IN ('individual','team')),
    starts_at TIMESTAMPTZ NOT NULL,
    ends_at TIMESTAMPTZ NOT NULL CHECK (ends_at > starts_at),
    tie_break TEXT NOT NULL CHECK (tie_break IN ('icpc','atcoder')),
    penalty_minutes INTEGER NOT NULL CHECK (penalty_minutes >= 0),
    revision TEXT NOT NULL CHECK (char_length(revision) BETWEEN 1 AND 256),
    participation_epoch UUID NOT NULL,
    team_salt UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    deleted_at TIMESTAMPTZ,
    CHECK (kind = 'regular' OR (participation_mode = 'individual' AND description = ''))
);
CREATE INDEX contests_public_page ON contests(kind,created_at,id) WHERE visibility = 'public' AND deleted_at IS NULL;

CREATE TABLE contest_problems (
    contest_id UUID NOT NULL REFERENCES contests(id),
    problem_id UUID NOT NULL REFERENCES problems(id),
    label TEXT NOT NULL CHECK (char_length(label) BETWEEN 1 AND 32),
    position INTEGER NOT NULL CHECK (position >= 0),
    removed_at TIMESTAMPTZ,
    PRIMARY KEY (contest_id,problem_id)
);
CREATE UNIQUE INDEX contest_problem_label ON contest_problems(contest_id,label) WHERE removed_at IS NULL;
CREATE UNIQUE INDEX contest_problem_position ON contest_problems(contest_id,position) WHERE removed_at IS NULL;
CREATE INDEX contest_problems_problem_id ON contest_problems(problem_id) WHERE removed_at IS NULL;

CREATE TABLE contest_teams (
    id UUID PRIMARY KEY,
    contest_id UUID NOT NULL REFERENCES contests(id),
    name TEXT NOT NULL CHECK (char_length(name) BETWEEN 1 AND 80),
    password_hash TEXT NOT NULL,
    UNIQUE (contest_id,name,password_hash),
    UNIQUE (contest_id,id)
);
CREATE TABLE contest_team_memberships (
    contest_id UUID NOT NULL REFERENCES contests(id),
    user_id UUID NOT NULL REFERENCES users(id),
    team_id UUID NOT NULL,
    joined_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (contest_id,user_id),
    FOREIGN KEY (contest_id,team_id) REFERENCES contest_teams(contest_id,id) ON DELETE CASCADE
);
CREATE INDEX contest_members_by_team ON contest_team_memberships(contest_id,team_id,user_id);

CREATE TABLE contest_request_receipts (
    user_id UUID NOT NULL REFERENCES users(id),
    operation TEXT NOT NULL CHECK (operation IN ('create-regular','create-virtual','join-individual','join-team')),
    scope_id UUID NOT NULL,
    key TEXT NOT NULL CHECK (char_length(key) BETWEEN 1 AND 128 AND key ~ '^[A-Za-z0-9._:-]+$'),
    request_digest BYTEA NOT NULL CHECK (octet_length(request_digest) = 32),
    participation_epoch UUID NOT NULL,
    result JSONB NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (user_id,operation,scope_id,key)
);
CREATE INDEX contest_receipts_expires ON contest_request_receipts(expires_at);

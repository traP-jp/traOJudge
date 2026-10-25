CREATE TABLE editorials (
    id UUID PRIMARY KEY,
    -- TODO(problems): 問題の永続化仕様が固まったらFKと削除時の扱いを定める。
    -- 未接続時は問題検査アダプターが503を返し、任意のUUIDを有効な問題と扱わない。
    problem_id UUID NOT NULL,
    author_id UUID NOT NULL REFERENCES users(id),
    title TEXT NOT NULL CHECK (char_length(title) BETWEEN 1 AND 200),
    body TEXT NOT NULL CHECK (char_length(body) BETWEEN 1 AND 200000),
    visibility TEXT NOT NULL CHECK (visibility IN ('public', 'private')),
    revision TEXT NOT NULL CHECK (char_length(revision) BETWEEN 1 AND 256),
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX editorials_public_page ON editorials(problem_id, created_at, id) WHERE visibility = 'public';

-- 本文の後日の編集に関わらず、同じ要求には作成時の表現を返す。
CREATE TABLE editorial_creation_receipts (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    key TEXT NOT NULL CHECK (char_length(key) BETWEEN 1 AND 128 AND key ~ '^[A-Za-z0-9._:-]+$'),
    request_digest BYTEA NOT NULL CHECK (octet_length(request_digest) = 32),
    editorial_id UUID NOT NULL REFERENCES editorials(id),
    problem_id UUID NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    revision TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (user_id, key)
);
CREATE INDEX editorial_creation_receipts_expires_at ON editorial_creation_receipts(expires_at);

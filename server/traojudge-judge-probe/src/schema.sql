CREATE TABLE IF NOT EXISTS judge_probe_order (
    scope TEXT PRIMARY KEY,
    accepted BIGINT NOT NULL DEFAULT 0,
    published BIGINT NOT NULL DEFAULT 0,
    CHECK (published <= accepted)
);
CREATE TABLE IF NOT EXISTS judge_probe_submissions (
    id UUID PRIMARY KEY,
    scope TEXT NOT NULL REFERENCES judge_probe_order(scope),
    ordinal BIGINT NOT NULL,
    definition JSONB NOT NULL,
    terminal BOOLEAN NOT NULL DEFAULT FALSE,
    error TEXT,
    reconciled_at TIMESTAMPTZ,
    published BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    finished_at TIMESTAMPTZ,
    UNIQUE(scope, ordinal)
);
CREATE TABLE IF NOT EXISTS judge_probe_queue (
    id TEXT PRIMARY KEY REFERENCES workflow_jobs(id),
    payload JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'ready' CHECK (status IN ('ready','leased','done','dead')),
    attempt INTEGER NOT NULL DEFAULT 0 CHECK (attempt BETWEEN 0 AND 3),
    token UUID,
    deadline TIMESTAMPTZ,
    result JSONB,
    applied BOOLEAN NOT NULL DEFAULT FALSE,
    worker TEXT,
    CHECK ((status = 'leased') = (deadline IS NOT NULL))
);
CREATE INDEX IF NOT EXISTS judge_probe_claim ON judge_probe_queue(status, deadline);

CREATE TABLE IF NOT EXISTS judge_probe_reserved_jobs (
    id TEXT PRIMARY KEY,
    submission UUID NOT NULL REFERENCES judge_probe_submissions(id)
);
CREATE OR REPLACE VIEW judge_probe_deadletters AS
    SELECT id::text AS id, error AS reason FROM judge_probe_submissions WHERE error IS NOT NULL
    UNION ALL
    SELECT id, result::text AS reason FROM judge_probe_queue WHERE status='dead';

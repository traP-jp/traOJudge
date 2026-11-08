CREATE TABLE judge_events (
 id BIGSERIAL PRIMARY KEY,
 run_id TEXT NOT NULL REFERENCES judge_runs(id),
 job_id TEXT NOT NULL REFERENCES workflow_jobs(id),
 node_name TEXT NOT NULL,
 kind TEXT NOT NULL CHECK (kind IN ('dispatched','result_received')),
 observations INTEGER NOT NULL DEFAULT 1,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 UNIQUE(job_id,kind)
);

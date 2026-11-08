
    CREATE TABLE workflow_pipelines (
        id TEXT PRIMARY KEY NOT NULL,
        state TEXT NOT NULL CHECK (state IN ('running', 'completed', 'cancelled'))
    )
;

    CREATE TABLE workflow_jobs (
        id TEXT PRIMARY KEY NOT NULL,
        pipeline_id TEXT NOT NULL REFERENCES workflow_pipelines(id),
        payload TEXT NOT NULL,
        state TEXT NOT NULL CHECK (
            state IN ('waiting', 'ready', 'starting', 'running', 'completed', 'failed', 'cancelled')
        )
    )
;

    CREATE TABLE workflow_edges (
        source_id TEXT NOT NULL REFERENCES workflow_jobs(id),
        target_id TEXT NOT NULL REFERENCES workflow_jobs(id),
        PRIMARY KEY (source_id, target_id)
    )
;
CREATE TABLE judge_runs (
 id TEXT PRIMARY KEY,
 definition JSONB NOT NULL,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE judge_artifacts (
 run_id TEXT NOT NULL REFERENCES judge_runs(id),
 name TEXT NOT NULL,
 reference JSONB NOT NULL,
 PRIMARY KEY(run_id, name)
);
CREATE TABLE judge_results (
 job_id TEXT PRIMARY KEY REFERENCES workflow_jobs(id),
 run_id TEXT NOT NULL REFERENCES judge_runs(id),
 node_name TEXT NOT NULL,
 body JSONB NOT NULL,
 applied BOOLEAN NOT NULL DEFAULT false,
 received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 UNIQUE(run_id, node_name)
);

CREATE TABLE workflow_pipelines (
    id TEXT PRIMARY KEY,
    state TEXT NOT NULL CHECK (state IN ('running', 'completed', 'cancelled'))
);
CREATE TABLE workflow_jobs (
    id TEXT PRIMARY KEY,
    pipeline_id TEXT NOT NULL REFERENCES workflow_pipelines(id),
    payload TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN
        ('waiting', 'ready', 'starting', 'running', 'completed', 'failed', 'cancelled'))
);
CREATE INDEX workflow_jobs_pipeline_state ON workflow_jobs(pipeline_id, state);
CREATE TABLE workflow_edges (
    source_id TEXT NOT NULL REFERENCES workflow_jobs(id),
    target_id TEXT NOT NULL REFERENCES workflow_jobs(id),
    PRIMARY KEY(source_id, target_id)
);
CREATE INDEX workflow_edges_target ON workflow_edges(target_id);

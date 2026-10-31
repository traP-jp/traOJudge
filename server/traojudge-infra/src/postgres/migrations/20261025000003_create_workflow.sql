CREATE TABLE workflow_pipelines (
    id UUID PRIMARY KEY NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('running', 'completed', 'cancelled'))
);

CREATE TABLE workflow_jobs (
    id UUID PRIMARY KEY NOT NULL,
    pipeline_id UUID NOT NULL REFERENCES workflow_pipelines(id),
    payload JSONB NOT NULL,
    state TEXT NOT NULL CHECK (
        state IN ('waiting', 'ready', 'starting', 'running', 'completed', 'failed', 'cancelled')
    )
);

CREATE INDEX workflow_jobs_pipeline_id_state_idx ON workflow_jobs (pipeline_id, state);

CREATE TABLE workflow_edges (
    source_id UUID NOT NULL REFERENCES workflow_jobs(id),
    target_id UUID NOT NULL REFERENCES workflow_jobs(id),
    PRIMARY KEY (source_id, target_id)
);

CREATE INDEX workflow_edges_target_id_idx ON workflow_edges (target_id);

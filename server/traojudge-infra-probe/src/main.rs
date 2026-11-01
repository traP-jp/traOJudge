//! Infrastructure adapter: one engine operation against a private checkpoint.
//! All callbacks are captured, never sent before the caller persists the outbox.
use std::{
    io,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::Row;
use traojudge_workflow::payload::{Payload, Revision0};
use traojudge_workflow::storage::inmemory::SnapshotUnitOfWorkProvider;
use traojudge_workflow::{Engine, JobId, JobStartCallback, PipelineId};

#[derive(Deserialize)]
struct Request {
    database: String,
    pipeline: PipelineId,
    jobs: Vec<(JobId, Value)>,
    edges: Vec<(JobId, JobId)>,
    #[serde(default)]
    completed: Option<JobId>,
    #[serde(default)]
    failed: bool,
}

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<Dispatch>>>);

#[derive(Serialize)]
struct Dispatch {
    id: JobId,
    task: Value,
}

#[async_trait]
impl JobStartCallback for Capture {
    type Input = Revision0<Value>;
    type Error = io::Error;
    async fn start(&self, id: JobId, payload: Payload<Self::Input>) -> io::Result<()> {
        self.0
            .lock()
            .map_err(|_| io::Error::other("poisoned outbox"))?
            .push(Dispatch {
                id,
                task: payload.into_value(),
            });
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let request: Request = serde_json::from_reader(io::stdin().lock())?;
    let provider = SnapshotUnitOfWorkProvider::open(&request.database).await?;
    let capture = Capture::default();
    let engine = Engine::new(provider.clone(), capture.clone());
    if let Some(job) = request.completed {
        if request.failed {
            engine.job_failed(job).await?;
        } else {
            engine.job_completed(job).await?;
        }
    } else {
        let jobs = request
            .jobs
            .into_iter()
            .map(|(id, task)| Ok((id, Payload::new(task)?)))
            .collect::<Result<Vec<_>>>()?;
        engine
            .start_pipeline(request.pipeline, jobs, &request.edges)
            .await?;
    }
    drop(engine);
    provider.close().await;
    let pool = sqlx::SqlitePool::connect(&format!("sqlite:{}", request.database)).await?;
    let pipeline: String = sqlx::query_scalar("SELECT state FROM workflow_pipelines WHERE id = ?")
        .bind(request.pipeline.to_string())
        .fetch_one(&pool)
        .await?;
    let rows = sqlx::query("SELECT id, state FROM workflow_jobs ORDER BY id")
        .fetch_all(&pool)
        .await?;
    let states = rows
        .into_iter()
        .map(|row| {
            Ok((
                row.try_get::<String, _>("id")?,
                row.try_get::<String, _>("state")?,
            ))
        })
        .collect::<Result<std::collections::BTreeMap<_, _>>>()?;
    pool.close().await;
    let dispatches = capture
        .0
        .lock()
        .map_err(|_| anyhow::anyhow!("poisoned outbox"))?;
    serde_json::to_writer(
        io::stdout().lock(),
        &serde_json::json!({
            "pipeline": pipeline, "states": states, "dispatches": *dispatches
        }),
    )
    .context("write engine result")?;
    Ok(())
}

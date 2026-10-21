use async_trait::async_trait;
use sqlx::SqliteConnection;

use crate::storage::{
    CompareAndSetPipelineStateError, CreatePipelineError, GetPipelineError, PipelineRepository,
};
use crate::{PipelineId, PipelineState};

pub(super) const CREATE_PIPELINES_TABLE: &str = "
    CREATE TABLE workflow_pipelines (
        id TEXT PRIMARY KEY NOT NULL,
        state TEXT NOT NULL CHECK (state IN ('running', 'completed', 'cancelled'))
    )
";

pub struct InMemoryPipelineRepository<'work> {
    connection: &'work mut SqliteConnection,
}

impl<'work> InMemoryPipelineRepository<'work> {
    pub(super) fn new(connection: &'work mut SqliteConnection) -> Self {
        Self { connection }
    }
}

#[async_trait]
impl PipelineRepository for InMemoryPipelineRepository<'_> {
    type Error = sqlx::Error;

    async fn create(
        &mut self,
        pipeline_id: PipelineId,
    ) -> Result<(), CreatePipelineError<Self::Error>> {
        let result = sqlx::query(
            "INSERT INTO workflow_pipelines (id, state) VALUES (?, 'running')
             ON CONFLICT(id) DO NOTHING",
        )
        .bind(pipeline_id.to_string())
        .execute(&mut *self.connection)
        .await?;

        if result.rows_affected() == 0 {
            return Err(CreatePipelineError::AlreadyExists(pipeline_id));
        }
        Ok(())
    }

    async fn get(
        &mut self,
        pipeline_id: PipelineId,
    ) -> Result<PipelineState, GetPipelineError<Self::Error>> {
        let stored: Option<String> =
            sqlx::query_scalar("SELECT state FROM workflow_pipelines WHERE id = ?")
                .bind(pipeline_id.to_string())
                .fetch_optional(&mut *self.connection)
                .await?;

        let state = stored.ok_or(GetPipelineError::NotFound(pipeline_id))?;
        serde_json::from_value(serde_json::Value::String(state))
            .map_err(|error| GetPipelineError::from(sqlx::Error::Decode(Box::new(error))))
    }

    async fn compare_and_set_state(
        &mut self,
        pipeline_id: PipelineId,
        expected: PipelineState,
        next: PipelineState,
    ) -> Result<(), CompareAndSetPipelineStateError<Self::Error>> {
        let result =
            sqlx::query("UPDATE workflow_pipelines SET state = ? WHERE id = ? AND state = ?")
                .bind(encode_state(next))
                .bind(pipeline_id.to_string())
                .bind(encode_state(expected))
                .execute(&mut *self.connection)
                .await?;

        if result.rows_affected() == 1 {
            return Ok(());
        }

        let actual = self.get(pipeline_id).await.map_err(|error| match error {
            GetPipelineError::NotFound(pipeline_id) => {
                CompareAndSetPipelineStateError::NotFound(pipeline_id)
            }
            GetPipelineError::Storage(error) => CompareAndSetPipelineStateError::Storage(error),
        })?;
        Err(CompareAndSetPipelineStateError::StateContradicts {
            pipeline_id,
            expected,
            actual,
        })
    }
}

fn encode_state(state: PipelineState) -> &'static str {
    match state {
        PipelineState::Running => "running",
        PipelineState::Completed => "completed",
        PipelineState::Cancelled => "cancelled",
    }
}

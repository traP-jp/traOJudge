use async_trait::async_trait;
use sqlx::SqliteConnection;

use crate::storage::{CompareAndSetJobStateError, CreateJobError, GetJobError, JobRepository};
use crate::{JobId, JobState};

pub(super) const CREATE_JOBS_TABLE: &str = "
    CREATE TABLE workflow_jobs (
        id TEXT PRIMARY KEY NOT NULL,
        state TEXT NOT NULL CHECK (
            state IN ('waiting', 'ready', 'starting', 'running', 'completed', 'failed', 'cancelled')
        )
    )
";

pub struct InMemoryJobRepository<'work> {
    connection: &'work mut SqliteConnection,
}

impl<'work> InMemoryJobRepository<'work> {
    pub(super) fn new(connection: &'work mut SqliteConnection) -> Self {
        Self { connection }
    }
}

#[async_trait]
impl JobRepository for InMemoryJobRepository<'_> {
    type Error = sqlx::Error;

    async fn create(&mut self, job_id: JobId) -> Result<(), CreateJobError<Self::Error>> {
        let result = sqlx::query(
            "INSERT INTO workflow_jobs (id, state) VALUES (?, 'waiting') ON CONFLICT(id) DO NOTHING",
        )
        .bind(job_id.to_string())
        .execute(&mut *self.connection)
        .await?;

        if result.rows_affected() == 0 {
            return Err(CreateJobError::AlreadyExists(job_id));
        }
        Ok(())
    }

    async fn get(&mut self, job_id: JobId) -> Result<JobState, GetJobError<Self::Error>> {
        let stored: Option<String> =
            sqlx::query_scalar("SELECT state FROM workflow_jobs WHERE id = ?")
                .bind(job_id.to_string())
                .fetch_optional(&mut *self.connection)
                .await?;

        let state = stored.ok_or(GetJobError::NotFound(job_id))?;
        serde_json::from_value(serde_json::Value::String(state))
            .map_err(|error| GetJobError::Storage(sqlx::Error::Decode(Box::new(error))))
    }

    async fn compare_and_set_state(
        &mut self,
        job_id: JobId,
        expected: JobState,
        next: JobState,
    ) -> Result<(), CompareAndSetJobStateError<Self::Error>> {
        let result = sqlx::query("UPDATE workflow_jobs SET state = ? WHERE id = ? AND state = ?")
            .bind(encode_state(next))
            .bind(job_id.to_string())
            .bind(encode_state(expected))
            .execute(&mut *self.connection)
            .await?;

        if result.rows_affected() == 1 {
            return Ok(());
        }

        let actual = self.get(job_id).await.map_err(|error| match error {
            GetJobError::NotFound(job_id) => CompareAndSetJobStateError::NotFound(job_id),
            GetJobError::Storage(error) => CompareAndSetJobStateError::Storage(error),
        })?;
        Err(CompareAndSetJobStateError::StateContradicts {
            job_id,
            expected,
            actual,
        })
    }
}

fn encode_state(state: JobState) -> &'static str {
    match state {
        JobState::Waiting => "waiting",
        JobState::Ready => "ready",
        JobState::Starting => "starting",
        JobState::Running => "running",
        JobState::Completed => "completed",
        JobState::Failed => "failed",
        JobState::Cancelled => "cancelled",
    }
}

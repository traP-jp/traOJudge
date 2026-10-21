use async_trait::async_trait;
use sqlx::{Connection, SqliteConnection};
use uuid::Uuid;

use crate::JobId;
use crate::storage::{EdgeRepository, ListNextJobsError, RegisterEdgesError};

pub(super) const CREATE_EDGES_TABLE: &str = "
    CREATE TABLE workflow_edges (
        source_id TEXT NOT NULL REFERENCES workflow_jobs(id),
        target_id TEXT NOT NULL REFERENCES workflow_jobs(id),
        PRIMARY KEY (source_id, target_id)
    )
";

pub struct InMemoryEdgeRepository<'work> {
    connection: &'work mut SqliteConnection,
}

impl<'work> InMemoryEdgeRepository<'work> {
    pub(super) fn new(connection: &'work mut SqliteConnection) -> Self {
        Self { connection }
    }
}

#[async_trait]
impl EdgeRepository for InMemoryEdgeRepository<'_> {
    type Error = sqlx::Error;

    async fn register(
        &mut self,
        edges: &[(JobId, JobId)],
    ) -> Result<(), RegisterEdgesError<Self::Error>> {
        let mut transaction = self.connection.begin().await?;

        for &(from, to) in edges {
            for job_id in [from, to] {
                let exists: bool =
                    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workflow_jobs WHERE id = ?)")
                        .bind(job_id.to_string())
                        .fetch_one(&mut *transaction)
                        .await?;
                if !exists {
                    return Err(RegisterEdgesError::JobNotFound(job_id));
                }
            }

            let result = sqlx::query(
                "INSERT INTO workflow_edges (source_id, target_id) VALUES (?, ?)
                 ON CONFLICT(source_id, target_id) DO NOTHING",
            )
            .bind(from.to_string())
            .bind(to.to_string())
            .execute(&mut *transaction)
            .await?;
            if result.rows_affected() == 0 {
                return Err(RegisterEdgesError::AlreadyExists { from, to });
            }
        }

        transaction.commit().await?;
        Ok(())
    }

    async fn list_next(
        &mut self,
        job_id: JobId,
    ) -> Result<Vec<JobId>, ListNextJobsError<Self::Error>> {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workflow_jobs WHERE id = ?)")
                .bind(job_id.to_string())
                .fetch_one(&mut *self.connection)
                .await?;
        if !exists {
            return Err(ListNextJobsError::JobNotFound(job_id));
        }

        let stored: Vec<String> = sqlx::query_scalar(
            "SELECT target_id FROM workflow_edges WHERE source_id = ? ORDER BY target_id",
        )
        .bind(job_id.to_string())
        .fetch_all(&mut *self.connection)
        .await?;

        stored
            .into_iter()
            .map(|value| {
                Uuid::parse_str(&value).map(JobId::from).map_err(|error| {
                    ListNextJobsError::Storage(sqlx::Error::Decode(Box::new(error)))
                })
            })
            .collect()
    }
}

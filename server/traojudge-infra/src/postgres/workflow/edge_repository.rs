use async_trait::async_trait;
use sqlx::{Connection, PgConnection};
use uuid::Uuid;

use traojudge_workflow::JobId;
use traojudge_workflow::storage::{
    CountIncomingEdgesError, EdgeRepository, ListNextJobsError, RegisterEdgesError,
    RemoveEdgesError,
};

pub struct PostgresEdgeRepository<'work> {
    connection: &'work mut PgConnection,
}

impl<'work> PostgresEdgeRepository<'work> {
    pub(super) fn new(connection: &'work mut PgConnection) -> Self {
        Self { connection }
    }
}

#[async_trait]
impl EdgeRepository for PostgresEdgeRepository<'_> {
    type Error = sqlx::Error;

    async fn register(
        &mut self,
        edges: &[(JobId, JobId)],
    ) -> Result<(), RegisterEdgesError<Self::Error>> {
        let mut transaction = self.connection.begin().await?;

        for &(from, to) in edges {
            for job_id in [from, to] {
                let exists: bool =
                    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workflow_jobs WHERE id = $1)")
                        .bind(*job_id.as_uuid())
                        .fetch_one(&mut *transaction)
                        .await?;
                if !exists {
                    transaction.rollback().await?;
                    return Err(RegisterEdgesError::JobNotFound(job_id));
                }
            }

            let result = sqlx::query(
                "INSERT INTO workflow_edges (source_id, target_id) VALUES ($1, $2)
                 ON CONFLICT(source_id, target_id) DO NOTHING",
            )
            .bind(*from.as_uuid())
            .bind(*to.as_uuid())
            .execute(&mut *transaction)
            .await?;
            if result.rows_affected() == 0 {
                transaction.rollback().await?;
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
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workflow_jobs WHERE id = $1)")
                .bind(*job_id.as_uuid())
                .fetch_one(&mut *self.connection)
                .await?;
        if !exists {
            return Err(ListNextJobsError::JobNotFound(job_id));
        }

        let stored: Vec<Uuid> = sqlx::query_scalar(
            "SELECT target_id FROM workflow_edges WHERE source_id = $1 ORDER BY target_id",
        )
        .bind(*job_id.as_uuid())
        .fetch_all(&mut *self.connection)
        .await?;

        Ok(stored.into_iter().map(JobId::from).collect())
    }

    async fn remove_from(
        &mut self,
        job_id: JobId,
    ) -> Result<Vec<JobId>, RemoveEdgesError<Self::Error>> {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workflow_jobs WHERE id = $1)")
                .bind(*job_id.as_uuid())
                .fetch_one(&mut *self.connection)
                .await?;
        if !exists {
            return Err(RemoveEdgesError::JobNotFound(job_id));
        }
        let stored: Vec<Uuid> = sqlx::query_scalar(
            "DELETE FROM workflow_edges WHERE source_id = $1 RETURNING target_id",
        )
        .bind(*job_id.as_uuid())
        .fetch_all(&mut *self.connection)
        .await?;
        let mut job_ids: Vec<_> = stored.into_iter().map(JobId::from).collect();
        job_ids.sort_by_key(|job_id| *job_id.as_uuid());
        Ok(job_ids)
    }

    async fn count_incoming(
        &mut self,
        job_id: JobId,
    ) -> Result<u64, CountIncomingEdgesError<Self::Error>> {
        let count: Option<i64> = sqlx::query_scalar(
            "SELECT COUNT(edges.source_id) FROM workflow_jobs AS jobs
             LEFT JOIN workflow_edges AS edges ON edges.target_id = jobs.id
             WHERE jobs.id = $1 GROUP BY jobs.id",
        )
        .bind(*job_id.as_uuid())
        .fetch_optional(&mut *self.connection)
        .await?;
        let count = count.ok_or(CountIncomingEdgesError::JobNotFound(job_id))?;
        u64::try_from(count)
            .map_err(|error| CountIncomingEdgesError::from(sqlx::Error::Decode(Box::new(error))))
    }
}

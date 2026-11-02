//! PostgreSQL adapter for the existing workflow Engine. Pipeline CAS takes a row
//! lock until commit, including Running -> Running, serializing graph changes.
use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
use sqlx::{Connection, PgConnection, PgPool, Postgres, Transaction};
use traojudge_workflow::{JobId, JobState, PipelineId, PipelineState, storage::*};
use uuid::Uuid;

#[derive(Clone)]
pub struct PgWorkflow(pub PgPool);

pub struct PgWork(Transaction<'static, Postgres>);
pub struct Repository<'a>(&'a mut PgConnection);

#[async_trait]
impl UnitOfWorkProvider for PgWorkflow {
    type Error = sqlx::Error;
    type UnitOfWork<'a> = PgWork;
    async fn begin(&self) -> Result<PgWork, Self::Error> {
        Ok(PgWork(self.0.begin().await?))
    }
}

#[async_trait]
impl UnitOfWork for PgWork {
    type Error = sqlx::Error;
    type JobRepository<'a> = Repository<'a>;
    type EdgeRepository<'a> = Repository<'a>;
    type PipelineRepository<'a> = Repository<'a>;
    fn jobs(&mut self) -> Repository<'_> {
        Repository(&mut self.0)
    }
    fn edges(&mut self) -> Repository<'_> {
        Repository(&mut self.0)
    }
    fn pipelines(&mut self) -> Repository<'_> {
        Repository(&mut self.0)
    }
    async fn commit(self) -> Result<(), Self::Error> {
        self.0.commit().await
    }
    async fn rollback(self) -> Result<(), Self::Error> {
        self.0.rollback().await
    }
}

fn encode<T: Serialize>(value: T) -> String {
    // Only the closed workflow state enums are passed here.
    serde_json::to_value(value)
        .expect("state serializes")
        .as_str()
        .expect("string state")
        .to_owned()
}
fn decode<T: DeserializeOwned>(value: String) -> Result<T, sqlx::Error> {
    serde_json::from_value(serde_json::Value::String(value))
        .map_err(|e| sqlx::Error::Decode(Box::new(e)))
}
fn job_id(value: String) -> Result<JobId, sqlx::Error> {
    Uuid::parse_str(&value)
        .map(JobId::from)
        .map_err(|e| sqlx::Error::Decode(Box::new(e)))
}

#[async_trait]
impl PipelineRepository for Repository<'_> {
    type Error = sqlx::Error;
    async fn create(&mut self, id: PipelineId) -> Result<(), CreatePipelineError<Self::Error>> {
        let result = sqlx::query(
            "INSERT INTO workflow_pipelines VALUES ($1, 'running') ON CONFLICT DO NOTHING",
        )
        .bind(id.to_string())
        .execute(&mut *self.0)
        .await?;
        if result.rows_affected() == 0 {
            return Err(CreatePipelineError::AlreadyExists(id));
        }
        Ok(())
    }
    async fn get(
        &mut self,
        id: PipelineId,
    ) -> Result<PipelineState, GetPipelineError<Self::Error>> {
        let state: Option<String> =
            sqlx::query_scalar("SELECT state FROM workflow_pipelines WHERE id=$1")
                .bind(id.to_string())
                .fetch_optional(&mut *self.0)
                .await?;
        Ok(decode(state.ok_or(GetPipelineError::NotFound(id))?)?)
    }
    async fn compare_and_set_state(
        &mut self,
        id: PipelineId,
        expected: PipelineState,
        next: PipelineState,
    ) -> Result<(), CompareAndSetPipelineStateError<Self::Error>> {
        let result = sqlx::query("UPDATE workflow_pipelines SET state=$3 WHERE id=$1 AND state=$2")
            .bind(id.to_string())
            .bind(encode(expected))
            .bind(encode(next))
            .execute(&mut *self.0)
            .await?;
        if result.rows_affected() == 1 {
            return Ok(());
        }
        let actual = PipelineRepository::get(self, id)
            .await
            .map_err(|e| match e {
                GetPipelineError::NotFound(id) => CompareAndSetPipelineStateError::NotFound(id),
                GetPipelineError::Storage(e) => CompareAndSetPipelineStateError::Storage(e),
            })?;
        Err(CompareAndSetPipelineStateError::StateContradicts {
            pipeline_id: id,
            expected,
            actual,
        })
    }
}

#[async_trait]
impl JobRepository for Repository<'_> {
    type Error = sqlx::Error;
    async fn create(
        &mut self,
        id: JobId,
        pipeline: PipelineId,
        payload: serde_json::Value,
    ) -> Result<(), CreateJobError<Self::Error>> {
        let result = sqlx::query(
            "INSERT INTO workflow_jobs (id,pipeline_id,payload,state)
            SELECT $1,$2,$3,'waiting' WHERE EXISTS(SELECT 1 FROM workflow_pipelines WHERE id=$2)
            ON CONFLICT DO NOTHING",
        )
        .bind(id.to_string())
        .bind(pipeline.to_string())
        .bind(payload.to_string())
        .execute(&mut *self.0)
        .await?;
        if result.rows_affected() == 0 {
            let exists: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workflow_jobs WHERE id=$1)")
                    .bind(id.to_string())
                    .fetch_one(&mut *self.0)
                    .await?;
            return Err(if exists {
                CreateJobError::AlreadyExists(id)
            } else {
                CreateJobError::PipelineNotFound(pipeline)
            });
        }
        Ok(())
    }
    async fn create_many(
        &mut self,
        jobs: &[(JobId, PipelineId, serde_json::Value)],
    ) -> Vec<Result<(), CreateJobError<Self::Error>>> {
        let mut out = Vec::with_capacity(jobs.len());
        for (id, pipeline, payload) in jobs {
            out.push(JobRepository::create(self, *id, *pipeline, payload.clone()).await);
        }
        out
    }
    async fn get(&mut self, id: JobId) -> Result<JobState, GetJobError<Self::Error>> {
        let state: Option<String> =
            sqlx::query_scalar("SELECT state FROM workflow_jobs WHERE id=$1")
                .bind(id.to_string())
                .fetch_optional(&mut *self.0)
                .await?;
        Ok(decode(state.ok_or(GetJobError::NotFound(id))?)?)
    }
    async fn get_many(&mut self, ids: &[JobId]) -> Vec<Result<JobState, GetJobError<Self::Error>>> {
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            out.push(JobRepository::get(self, *id).await);
        }
        out
    }
    async fn get_pipeline_id(
        &mut self,
        id: JobId,
    ) -> Result<PipelineId, GetJobPipelineIdError<Self::Error>> {
        let pipeline: Option<String> =
            sqlx::query_scalar("SELECT pipeline_id FROM workflow_jobs WHERE id=$1")
                .bind(id.to_string())
                .fetch_optional(&mut *self.0)
                .await?;
        let value = pipeline.ok_or(GetJobPipelineIdError::NotFound(id))?;
        Ok(PipelineId::from(
            Uuid::parse_str(&value).map_err(|e| sqlx::Error::Decode(Box::new(e)))?,
        ))
    }
    async fn count(
        &mut self,
        pipeline: PipelineId,
        states: &[JobState],
    ) -> Result<u64, CountJobsError<Self::Error>> {
        let states: Vec<_> = states.iter().map(encode).collect();
        let value: Option<i64> = sqlx::query_scalar("SELECT COUNT(j.id) FROM workflow_pipelines p
            LEFT JOIN workflow_jobs j ON j.pipeline_id=p.id AND j.state=ANY($2) WHERE p.id=$1 GROUP BY p.id")
            .bind(pipeline.to_string()).bind(states).fetch_optional(&mut *self.0).await?;
        Ok(value.ok_or(CountJobsError::PipelineNotFound(pipeline))? as u64)
    }
    async fn list_by_state(
        &mut self,
        pipeline: PipelineId,
        states: &[JobState],
    ) -> Result<Vec<JobId>, ListJobsError<Self::Error>> {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workflow_pipelines WHERE id=$1)")
                .bind(pipeline.to_string())
                .fetch_one(&mut *self.0)
                .await?;
        if !exists {
            return Err(ListJobsError::PipelineNotFound(pipeline));
        }
        let states: Vec<_> = states.iter().map(encode).collect();
        let ids: Vec<String> = sqlx::query_scalar(
            "SELECT id FROM workflow_jobs WHERE pipeline_id=$1 AND state=ANY($2) ORDER BY id",
        )
        .bind(pipeline.to_string())
        .bind(states)
        .fetch_all(&mut *self.0)
        .await?;
        Ok(ids.into_iter().map(job_id).collect::<Result<_, _>>()?)
    }
    async fn get_payload(
        &mut self,
        id: JobId,
    ) -> Result<serde_json::Value, GetJobPayloadError<Self::Error>> {
        let payload: Option<String> =
            sqlx::query_scalar("SELECT payload FROM workflow_jobs WHERE id=$1")
                .bind(id.to_string())
                .fetch_optional(&mut *self.0)
                .await?;
        serde_json::from_str(&payload.ok_or(GetJobPayloadError::NotFound(id))?)
            .map_err(|e| sqlx::Error::Decode(Box::new(e)).into())
    }
    async fn compare_and_set_state(
        &mut self,
        id: JobId,
        expected: JobState,
        next: JobState,
    ) -> Result<(), CompareAndSetJobStateError<Self::Error>> {
        let result = sqlx::query("UPDATE workflow_jobs SET state=$3 WHERE id=$1 AND state=$2")
            .bind(id.to_string())
            .bind(encode(expected))
            .bind(encode(next))
            .execute(&mut *self.0)
            .await?;
        if result.rows_affected() == 1 {
            return Ok(());
        }
        let actual = JobRepository::get(self, id).await.map_err(|e| match e {
            GetJobError::NotFound(id) => CompareAndSetJobStateError::NotFound(id),
            GetJobError::Storage(e) => CompareAndSetJobStateError::Storage(e),
        })?;
        Err(CompareAndSetJobStateError::StateContradicts {
            job_id: id,
            expected,
            actual,
        })
    }
    async fn compare_and_set_state_many(
        &mut self,
        updates: &[(JobId, JobState, JobState)],
    ) -> Vec<Result<(), CompareAndSetJobStateError<Self::Error>>> {
        let mut out = Vec::with_capacity(updates.len());
        for (id, expected, next) in updates {
            out.push(JobRepository::compare_and_set_state(self, *id, *expected, *next).await);
        }
        out
    }
}

#[async_trait]
impl EdgeRepository for Repository<'_> {
    type Error = sqlx::Error;
    async fn register(
        &mut self,
        edges: &[(JobId, JobId)],
    ) -> Result<(), RegisterEdgesError<Self::Error>> {
        // A savepoint preserves the repository contract: an invalid batch changes nothing.
        let mut tx = self.0.begin().await?;
        for (from, to) in edges {
            for id in [from, to] {
                let exists: bool =
                    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workflow_jobs WHERE id=$1)")
                        .bind(id.to_string())
                        .fetch_one(&mut *tx)
                        .await?;
                if !exists {
                    return Err(RegisterEdgesError::JobNotFound(*id));
                }
            }
            let result =
                sqlx::query("INSERT INTO workflow_edges VALUES ($1,$2) ON CONFLICT DO NOTHING")
                    .bind(from.to_string())
                    .bind(to.to_string())
                    .execute(&mut *tx)
                    .await?;
            if result.rows_affected() == 0 {
                return Err(RegisterEdgesError::AlreadyExists {
                    from: *from,
                    to: *to,
                });
            }
        }
        tx.commit().await?;
        Ok(())
    }
    async fn list_next(&mut self, id: JobId) -> Result<Vec<JobId>, ListNextJobsError<Self::Error>> {
        if !self.job_exists(id).await? {
            return Err(ListNextJobsError::JobNotFound(id));
        }
        let ids: Vec<String> = sqlx::query_scalar(
            "SELECT target_id FROM workflow_edges WHERE source_id=$1 ORDER BY target_id",
        )
        .bind(id.to_string())
        .fetch_all(&mut *self.0)
        .await?;
        Ok(ids.into_iter().map(job_id).collect::<Result<_, _>>()?)
    }
    async fn remove_from(
        &mut self,
        id: JobId,
    ) -> Result<Vec<JobId>, RemoveEdgesError<Self::Error>> {
        if !self.job_exists(id).await? {
            return Err(RemoveEdgesError::JobNotFound(id));
        }
        let ids: Vec<String> =
            sqlx::query_scalar("DELETE FROM workflow_edges WHERE source_id=$1 RETURNING target_id")
                .bind(id.to_string())
                .fetch_all(&mut *self.0)
                .await?;
        let mut ids = ids.into_iter().map(job_id).collect::<Result<Vec<_>, _>>()?;
        ids.sort_by_key(|id| *id.as_uuid());
        Ok(ids)
    }
    async fn count_incoming(
        &mut self,
        id: JobId,
    ) -> Result<u64, CountIncomingEdgesError<Self::Error>> {
        if !self.job_exists(id).await? {
            return Err(CountIncomingEdgesError::JobNotFound(id));
        }
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workflow_edges WHERE target_id=$1")
            .bind(id.to_string())
            .fetch_one(&mut *self.0)
            .await?;
        Ok(n as u64)
    }
}
impl Repository<'_> {
    async fn job_exists(&mut self, id: JobId) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workflow_jobs WHERE id=$1)")
            .bind(id.to_string())
            .fetch_one(&mut *self.0)
            .await
    }
}

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use sqlx::{QueryBuilder, Sqlite, SqliteConnection};
use uuid::Uuid;

use crate::storage::{
    CompareAndSetJobStateError, CountJobsError, CreateJobError, GetJobError, GetJobPayloadError,
    GetJobPipelineIdError, JobRepository, ListJobsError,
};
use crate::{JobId, JobState, PipelineId};

const JOB_BATCH_SIZE: usize = 256;

pub(super) const CREATE_JOBS_TABLE: &str = "
    CREATE TABLE workflow_jobs (
        id TEXT PRIMARY KEY NOT NULL,
        pipeline_id TEXT NOT NULL REFERENCES workflow_pipelines(id),
        payload TEXT NOT NULL,
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

    async fn create(
        &mut self,
        job_id: JobId,
        pipeline_id: PipelineId,
        payload: serde_json::Value,
    ) -> Result<(), CreateJobError<Self::Error>> {
        let result = sqlx::query(
            "INSERT INTO workflow_jobs (id, pipeline_id, payload, state)
             SELECT ?, ?, ?, 'waiting' WHERE EXISTS (SELECT 1 FROM workflow_pipelines WHERE id = ?)
             ON CONFLICT(id) DO NOTHING",
        )
        .bind(job_id.to_string())
        .bind(pipeline_id.to_string())
        .bind(payload.to_string())
        .bind(pipeline_id.to_string())
        .execute(&mut *self.connection)
        .await?;

        if result.rows_affected() == 0 {
            let exists: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workflow_jobs WHERE id = ?)")
                    .bind(job_id.to_string())
                    .fetch_one(&mut *self.connection)
                    .await?;
            return Err(if exists {
                CreateJobError::AlreadyExists(job_id)
            } else {
                CreateJobError::PipelineNotFound(pipeline_id)
            });
        }
        Ok(())
    }

    async fn create_many(
        &mut self,
        jobs: &[(JobId, PipelineId, serde_json::Value)],
    ) -> Vec<Result<(), CreateJobError<Self::Error>>> {
        let mut results = Vec::with_capacity(jobs.len());
        for batch in jobs.chunks(JOB_BATCH_SIZE) {
            match create_jobs(self.connection, batch).await {
                Ok(created) => results.extend(created),
                Err(error) => {
                    let error = Arc::new(error);
                    results.extend(
                        batch
                            .iter()
                            .map(|_| Err(CreateJobError::Storage(error.clone()))),
                    );
                }
            }
        }
        results
    }

    async fn get(&mut self, job_id: JobId) -> Result<JobState, GetJobError<Self::Error>> {
        let stored: Option<String> =
            sqlx::query_scalar("SELECT state FROM workflow_jobs WHERE id = ?")
                .bind(job_id.to_string())
                .fetch_optional(&mut *self.connection)
                .await?;

        let state = stored.ok_or(GetJobError::NotFound(job_id))?;
        decode_state(state).map_err(GetJobError::from)
    }

    async fn get_many(
        &mut self,
        job_ids: &[JobId],
    ) -> Vec<Result<JobState, GetJobError<Self::Error>>> {
        let mut results = Vec::with_capacity(job_ids.len());
        for batch in job_ids.chunks(JOB_BATCH_SIZE) {
            match load_states(self.connection, batch).await {
                Ok(states) => results.extend(batch.iter().map(|job_id| {
                    states
                        .get(job_id)
                        .copied()
                        .ok_or(GetJobError::NotFound(*job_id))
                })),
                Err(error) => {
                    let error = Arc::new(error);
                    results.extend(
                        batch
                            .iter()
                            .map(|_| Err(GetJobError::Storage(error.clone()))),
                    );
                }
            }
        }
        results
    }

    async fn get_pipeline_id(
        &mut self,
        job_id: JobId,
    ) -> Result<PipelineId, GetJobPipelineIdError<Self::Error>> {
        let stored: Option<String> =
            sqlx::query_scalar("SELECT pipeline_id FROM workflow_jobs WHERE id = ?")
                .bind(job_id.to_string())
                .fetch_optional(&mut *self.connection)
                .await?;
        let pipeline_id = stored.ok_or(GetJobPipelineIdError::NotFound(job_id))?;
        Uuid::parse_str(&pipeline_id)
            .map(PipelineId::from)
            .map_err(|error| GetJobPipelineIdError::from(sqlx::Error::Decode(Box::new(error))))
    }

    async fn count(
        &mut self,
        pipeline_id: PipelineId,
        states: &[JobState],
    ) -> Result<u64, CountJobsError<Self::Error>> {
        let mut query = QueryBuilder::<Sqlite>::new(
            "SELECT COUNT(jobs.id) FROM workflow_pipelines AS pipelines
             LEFT JOIN workflow_jobs AS jobs ON jobs.pipeline_id = pipelines.id AND ",
        );
        if states.is_empty() {
            query.push("1 = 0");
        } else {
            query.push("jobs.state IN (");
            let mut values = query.separated(", ");
            for &state in states {
                values.push_bind(encode_state(state));
            }
            query.push(")");
        }
        query
            .push(" WHERE pipelines.id = ")
            .push_bind(pipeline_id.to_string())
            .push(" GROUP BY pipelines.id");
        let count = query
            .build_query_scalar::<i64>()
            .fetch_optional(&mut *self.connection)
            .await?
            .ok_or(CountJobsError::PipelineNotFound(pipeline_id))?;
        u64::try_from(count)
            .map_err(|error| CountJobsError::from(sqlx::Error::Decode(Box::new(error))))
    }

    async fn list_by_state(
        &mut self,
        pipeline_id: PipelineId,
        states: &[JobState],
    ) -> Result<Vec<JobId>, ListJobsError<Self::Error>> {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workflow_pipelines WHERE id = ?)")
                .bind(pipeline_id.to_string())
                .fetch_one(&mut *self.connection)
                .await?;
        if !exists {
            return Err(ListJobsError::PipelineNotFound(pipeline_id));
        }
        if states.is_empty() {
            return Ok(Vec::new());
        }
        let mut query =
            QueryBuilder::<Sqlite>::new("SELECT id FROM workflow_jobs WHERE pipeline_id = ");
        query
            .push_bind(pipeline_id.to_string())
            .push(" AND state IN (");
        let mut values = query.separated(", ");
        for &state in states {
            values.push_bind(encode_state(state));
        }
        query.push(") ORDER BY id");
        query
            .build_query_scalar::<String>()
            .fetch_all(&mut *self.connection)
            .await?
            .into_iter()
            .map(|id| {
                Uuid::parse_str(&id)
                    .map(JobId::from)
                    .map_err(|error| ListJobsError::from(sqlx::Error::Decode(Box::new(error))))
            })
            .collect()
    }

    async fn get_payload(
        &mut self,
        job_id: JobId,
    ) -> Result<serde_json::Value, GetJobPayloadError<Self::Error>> {
        let stored: Option<String> =
            sqlx::query_scalar("SELECT payload FROM workflow_jobs WHERE id = ?")
                .bind(job_id.to_string())
                .fetch_optional(&mut *self.connection)
                .await?;
        let payload = stored.ok_or(GetJobPayloadError::NotFound(job_id))?;
        serde_json::from_str(&payload)
            .map_err(|error| GetJobPayloadError::from(sqlx::Error::Decode(Box::new(error))))
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

    async fn compare_and_set_state_many(
        &mut self,
        updates: &[(JobId, JobState, JobState)],
    ) -> Vec<Result<(), CompareAndSetJobStateError<Self::Error>>> {
        let mut results = Vec::with_capacity(updates.len());
        let mut start = 0;
        while start < updates.len() {
            let mut seen = HashSet::new();
            let mut end = start;
            while end < updates.len() && end - start < JOB_BATCH_SIZE && seen.insert(updates[end].0)
            {
                end += 1;
            }
            results.extend(update_states(self.connection, &updates[start..end]).await);
            start = end;
        }
        results
    }
}

async fn create_jobs(
    connection: &mut SqliteConnection,
    jobs: &[(JobId, PipelineId, serde_json::Value)],
) -> Result<Vec<Result<(), CreateJobError<sqlx::Error>>>, sqlx::Error> {
    let job_ids: Vec<_> = jobs.iter().map(|(job_id, _, _)| *job_id).collect();
    let mut existing_jobs: HashSet<_> = load_states(connection, &job_ids)
        .await?
        .into_keys()
        .collect();
    let mut query = QueryBuilder::<Sqlite>::new("SELECT id FROM workflow_pipelines WHERE id IN (");
    let mut ids = query.separated(", ");
    for (_, pipeline_id, _) in jobs {
        ids.push_bind(pipeline_id.to_string());
    }
    query.push(")");
    let pipelines: HashSet<_> = query
        .build_query_scalar::<String>()
        .fetch_all(&mut *connection)
        .await?
        .into_iter()
        .collect();
    let valid_jobs: Vec<_> = jobs
        .iter()
        .filter(|(_, pipeline_id, _)| pipelines.contains(&pipeline_id.to_string()))
        .collect();
    let mut inserted = HashSet::new();
    if !valid_jobs.is_empty() {
        let mut query = QueryBuilder::<Sqlite>::new(
            "INSERT INTO workflow_jobs (id, pipeline_id, payload, state) ",
        );
        query.push_values(&valid_jobs, |mut row, job| {
            row.push_bind(job.0.to_string())
                .push_bind(job.1.to_string())
                .push_bind(job.2.to_string())
                .push_bind("waiting");
        });
        query.push(" ON CONFLICT(id) DO NOTHING RETURNING id");
        inserted.extend(
            query
                .build_query_scalar::<String>()
                .fetch_all(&mut *connection)
                .await?,
        );
    }
    Ok(jobs
        .iter()
        .map(|&(job_id, pipeline_id, _)| {
            if existing_jobs.contains(&job_id) {
                Err(CreateJobError::AlreadyExists(job_id))
            } else if !pipelines.contains(&pipeline_id.to_string()) {
                Err(CreateJobError::PipelineNotFound(pipeline_id))
            } else if inserted.remove(&job_id.to_string()) {
                existing_jobs.insert(job_id);
                Ok(())
            } else {
                Err(CreateJobError::AlreadyExists(job_id))
            }
        })
        .collect())
}

async fn load_states(
    connection: &mut SqliteConnection,
    job_ids: &[JobId],
) -> Result<HashMap<JobId, JobState>, sqlx::Error> {
    let mut query =
        QueryBuilder::<Sqlite>::new("SELECT id, state FROM workflow_jobs WHERE id IN (");
    let mut ids = query.separated(", ");
    for job_id in job_ids {
        ids.push_bind(job_id.to_string());
    }
    query.push(")");
    let rows = query
        .build_query_as::<(String, String)>()
        .fetch_all(connection)
        .await?;
    rows.into_iter()
        .map(|(id, state)| {
            let id = Uuid::parse_str(&id).map_err(|error| sqlx::Error::Decode(Box::new(error)))?;
            Ok((JobId::from(id), decode_state(state)?))
        })
        .collect()
}

async fn update_states(
    connection: &mut SqliteConnection,
    updates: &[(JobId, JobState, JobState)],
) -> Vec<Result<(), CompareAndSetJobStateError<sqlx::Error>>> {
    let job_ids: Vec<_> = updates.iter().map(|&(job_id, _, _)| job_id).collect();
    let states = match load_states(connection, &job_ids).await {
        Ok(states) => states,
        Err(error) => {
            let error = Arc::new(error);
            return updates
                .iter()
                .map(|_| Err(CompareAndSetJobStateError::Storage(error.clone())))
                .collect();
        }
    };
    let mut results: Vec<_> = updates
        .iter()
        .map(
            |&(job_id, expected, _)| match states.get(&job_id).copied() {
                None => Err(CompareAndSetJobStateError::NotFound(job_id)),
                Some(actual) if actual != expected => {
                    Err(CompareAndSetJobStateError::StateContradicts {
                        job_id,
                        expected,
                        actual,
                    })
                }
                Some(_) => Ok(()),
            },
        )
        .collect();
    let changes: Vec<_> = updates
        .iter()
        .copied()
        .enumerate()
        .filter(|(index, _)| results[*index].is_ok())
        .collect();
    if changes.is_empty() {
        return results;
    }
    let mut query =
        QueryBuilder::<Sqlite>::new("WITH requested(id, expected_state, next_state) AS (");
    query.push_values(&changes, |mut row, &(_, (job_id, expected, next))| {
        row.push_bind(job_id.to_string())
            .push_bind(encode_state(expected))
            .push_bind(encode_state(next));
    });
    query.push(
        ") UPDATE workflow_jobs SET state = requested.next_state FROM requested
        WHERE workflow_jobs.id = requested.id AND workflow_jobs.state = requested.expected_state
        RETURNING workflow_jobs.id",
    );
    let updated = match query
        .build_query_as::<(String,)>()
        .fetch_all(&mut *connection)
        .await
    {
        Ok(rows) => rows.into_iter().map(|(id,)| id).collect::<HashSet<_>>(),
        Err(error) => {
            let error = Arc::new(error);
            for &(index, _) in &changes {
                results[index] = Err(CompareAndSetJobStateError::Storage(error.clone()));
            }
            return results;
        }
    };
    let missed: Vec<_> = changes
        .into_iter()
        .filter(|&(_, (job_id, _, _))| !updated.contains(&job_id.to_string()))
        .collect();
    if !missed.is_empty() {
        let job_ids: Vec<_> = missed.iter().map(|&(_, (job_id, _, _))| job_id).collect();
        match load_states(connection, &job_ids).await {
            Ok(states) => {
                for (index, (job_id, expected, _)) in missed {
                    results[index] = Err(match states.get(&job_id).copied() {
                        Some(actual) => CompareAndSetJobStateError::StateContradicts {
                            job_id,
                            expected,
                            actual,
                        },
                        None => CompareAndSetJobStateError::NotFound(job_id),
                    });
                }
            }
            Err(error) => {
                let error = Arc::new(error);
                for (index, _) in missed {
                    results[index] = Err(CompareAndSetJobStateError::Storage(error.clone()));
                }
            }
        }
    }
    results
}

fn decode_state(state: String) -> Result<JobState, sqlx::Error> {
    serde_json::from_value(serde_json::Value::String(state))
        .map_err(|error| sqlx::Error::Decode(Box::new(error)))
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

#[cfg(test)]
mod tests;

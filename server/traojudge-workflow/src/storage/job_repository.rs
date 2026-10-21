use std::sync::Arc;

use async_trait::async_trait;

use crate::{JobId, JobState, PipelineId};

#[async_trait]
pub trait JobRepository: Send {
    type Error: std::error::Error + Send + Sync + 'static;

    async fn create(
        &mut self,
        job_id: JobId,
        pipeline_id: PipelineId,
        payload: serde_json::Value,
    ) -> Result<(), CreateJobError<Self::Error>>;

    async fn create_many(
        &mut self,
        jobs: &[(JobId, PipelineId, serde_json::Value)],
    ) -> Vec<Result<(), CreateJobError<Self::Error>>>;

    async fn get(&mut self, job_id: JobId) -> Result<JobState, GetJobError<Self::Error>>;

    async fn get_many(
        &mut self,
        job_ids: &[JobId],
    ) -> Vec<Result<JobState, GetJobError<Self::Error>>>;

    async fn get_pipeline_id(
        &mut self,
        job_id: JobId,
    ) -> Result<PipelineId, GetJobPipelineIdError<Self::Error>>;

    async fn count(
        &mut self,
        pipeline_id: PipelineId,
        states: &[JobState],
    ) -> Result<u64, CountJobsError<Self::Error>>;

    async fn list_by_state(
        &mut self,
        pipeline_id: PipelineId,
        states: &[JobState],
    ) -> Result<Vec<JobId>, ListJobsError<Self::Error>>;

    async fn get_payload(
        &mut self,
        job_id: JobId,
    ) -> Result<serde_json::Value, GetJobPayloadError<Self::Error>>;

    async fn compare_and_set_state(
        &mut self,
        job_id: JobId,
        expected: JobState,
        next: JobState,
    ) -> Result<(), CompareAndSetJobStateError<Self::Error>>;

    async fn compare_and_set_state_many(
        &mut self,
        updates: &[(JobId, JobState, JobState)],
    ) -> Vec<Result<(), CompareAndSetJobStateError<Self::Error>>>;
}

#[derive(Debug, thiserror::Error)]
pub enum CreateJobError<StorageError: std::error::Error + 'static> {
    #[error("job {0} already exists")]
    AlreadyExists(JobId),
    #[error("pipeline {0} not found")]
    PipelineNotFound(PipelineId),
    #[error(transparent)]
    Storage(Arc<StorageError>),
}

#[derive(Debug, thiserror::Error)]
pub enum GetJobError<StorageError: std::error::Error + 'static> {
    #[error("job {0} not found")]
    NotFound(JobId),
    #[error(transparent)]
    Storage(Arc<StorageError>),
}

#[derive(Debug, thiserror::Error)]
pub enum GetJobPipelineIdError<StorageError: std::error::Error + 'static> {
    #[error("job {0} not found")]
    NotFound(JobId),
    #[error(transparent)]
    Storage(Arc<StorageError>),
}

#[derive(Debug, thiserror::Error)]
pub enum CountJobsError<StorageError: std::error::Error + 'static> {
    #[error("pipeline {0} not found")]
    PipelineNotFound(PipelineId),
    #[error(transparent)]
    Storage(Arc<StorageError>),
}

#[derive(Debug, thiserror::Error)]
pub enum ListJobsError<StorageError: std::error::Error + 'static> {
    #[error("pipeline {0} not found")]
    PipelineNotFound(PipelineId),
    #[error(transparent)]
    Storage(Arc<StorageError>),
}

#[derive(Debug, thiserror::Error)]
pub enum GetJobPayloadError<StorageError: std::error::Error + 'static> {
    #[error("job {0} not found")]
    NotFound(JobId),
    #[error(transparent)]
    Storage(Arc<StorageError>),
}

#[derive(Debug, thiserror::Error)]
pub enum CompareAndSetJobStateError<StorageError: std::error::Error + 'static> {
    #[error("job {0} not found")]
    NotFound(JobId),
    #[error("job {job_id} state contradicts expected {expected:?}: actual state is {actual:?}")]
    StateContradicts {
        job_id: JobId,
        expected: JobState,
        actual: JobState,
    },
    #[error(transparent)]
    Storage(Arc<StorageError>),
}

impl<StorageError: std::error::Error + 'static> From<StorageError>
    for CreateJobError<StorageError>
{
    fn from(error: StorageError) -> Self {
        Self::Storage(Arc::new(error))
    }
}

impl<StorageError: std::error::Error + 'static> From<StorageError> for GetJobError<StorageError> {
    fn from(error: StorageError) -> Self {
        Self::Storage(Arc::new(error))
    }
}

impl<StorageError: std::error::Error + 'static> From<StorageError>
    for GetJobPipelineIdError<StorageError>
{
    fn from(error: StorageError) -> Self {
        Self::Storage(Arc::new(error))
    }
}

impl<StorageError: std::error::Error + 'static> From<StorageError>
    for CountJobsError<StorageError>
{
    fn from(error: StorageError) -> Self {
        Self::Storage(Arc::new(error))
    }
}

impl<StorageError: std::error::Error + 'static> From<StorageError> for ListJobsError<StorageError> {
    fn from(error: StorageError) -> Self {
        Self::Storage(Arc::new(error))
    }
}

impl<StorageError: std::error::Error + 'static> From<StorageError>
    for GetJobPayloadError<StorageError>
{
    fn from(error: StorageError) -> Self {
        Self::Storage(Arc::new(error))
    }
}

impl<StorageError: std::error::Error + 'static> From<StorageError>
    for CompareAndSetJobStateError<StorageError>
{
    fn from(error: StorageError) -> Self {
        Self::Storage(Arc::new(error))
    }
}

use async_trait::async_trait;

use crate::{JobId, JobState};

#[async_trait]
pub trait JobRepository: Send {
    type Error: std::error::Error + Send + Sync + 'static;

    async fn create(&mut self, job_id: JobId) -> Result<(), CreateJobError<Self::Error>>;

    async fn get(&mut self, job_id: JobId) -> Result<JobState, GetJobError<Self::Error>>;

    async fn compare_and_set_state(
        &mut self,
        job_id: JobId,
        expected: JobState,
        next: JobState,
    ) -> Result<(), CompareAndSetJobStateError<Self::Error>>;
}

#[derive(Debug, thiserror::Error)]
pub enum CreateJobError<StorageError: std::error::Error + 'static> {
    #[error("job {0} already exists")]
    AlreadyExists(JobId),
    #[error(transparent)]
    Storage(#[from] StorageError),
}

#[derive(Debug, thiserror::Error)]
pub enum GetJobError<StorageError: std::error::Error + 'static> {
    #[error("job {0} not found")]
    NotFound(JobId),
    #[error(transparent)]
    Storage(#[from] StorageError),
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
    Storage(#[from] StorageError),
}

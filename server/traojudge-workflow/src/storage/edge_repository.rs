use async_trait::async_trait;

use crate::JobId;

#[async_trait]
pub trait EdgeRepository: Send {
    type Error: std::error::Error + Send + Sync + 'static;

    async fn register(
        &mut self,
        edges: &[(JobId, JobId)],
    ) -> Result<(), RegisterEdgesError<Self::Error>>;

    async fn list_next(
        &mut self,
        job_id: JobId,
    ) -> Result<Vec<JobId>, ListNextJobsError<Self::Error>>;
}

#[derive(Debug, thiserror::Error)]
pub enum RegisterEdgesError<StorageError: std::error::Error + 'static> {
    #[error("edge from {from} to {to} already exists")]
    AlreadyExists { from: JobId, to: JobId },
    #[error("job {0} not found")]
    JobNotFound(JobId),
    #[error(transparent)]
    Storage(#[from] StorageError),
}

#[derive(Debug, thiserror::Error)]
pub enum ListNextJobsError<StorageError: std::error::Error + 'static> {
    #[error("job {0} not found")]
    JobNotFound(JobId),
    #[error(transparent)]
    Storage(#[from] StorageError),
}

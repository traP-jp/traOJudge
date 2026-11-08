use std::sync::Arc;

use async_trait::async_trait;

use crate::{PipelineId, PipelineState};

#[async_trait]
pub trait PipelineRepository: Send {
    type Error: std::error::Error + Send + Sync + 'static;

    async fn create(
        &mut self,
        pipeline_id: PipelineId,
    ) -> Result<(), CreatePipelineError<Self::Error>>;

    async fn get(
        &mut self,
        pipeline_id: PipelineId,
    ) -> Result<PipelineState, GetPipelineError<Self::Error>>;

    async fn compare_and_set_state(
        &mut self,
        pipeline_id: PipelineId,
        expected: PipelineState,
        next: PipelineState,
    ) -> Result<(), CompareAndSetPipelineStateError<Self::Error>>;
}

#[derive(Debug, thiserror::Error)]
pub enum CreatePipelineError<StorageError: std::error::Error + 'static> {
    #[error("pipeline {0} already exists")]
    AlreadyExists(PipelineId),
    #[error(transparent)]
    Storage(Arc<StorageError>),
}

#[derive(Debug, thiserror::Error)]
pub enum GetPipelineError<StorageError: std::error::Error + 'static> {
    #[error("pipeline {0} not found")]
    NotFound(PipelineId),
    #[error(transparent)]
    Storage(Arc<StorageError>),
}

#[derive(Debug, thiserror::Error)]
pub enum CompareAndSetPipelineStateError<StorageError: std::error::Error + 'static> {
    #[error("pipeline {0} not found")]
    NotFound(PipelineId),
    #[error(
        "pipeline {pipeline_id} state contradicts expected {expected:?}: actual state is {actual:?}"
    )]
    StateContradicts {
        pipeline_id: PipelineId,
        expected: PipelineState,
        actual: PipelineState,
    },
    #[error(transparent)]
    Storage(Arc<StorageError>),
}

impl<StorageError: std::error::Error + 'static> From<StorageError>
    for CreatePipelineError<StorageError>
{
    fn from(error: StorageError) -> Self {
        Self::Storage(Arc::new(error))
    }
}

impl<StorageError: std::error::Error + 'static> From<StorageError>
    for GetPipelineError<StorageError>
{
    fn from(error: StorageError) -> Self {
        Self::Storage(Arc::new(error))
    }
}

impl<StorageError: std::error::Error + 'static> From<StorageError>
    for CompareAndSetPipelineStateError<StorageError>
{
    fn from(error: StorageError) -> Self {
        Self::Storage(Arc::new(error))
    }
}

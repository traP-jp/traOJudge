use std::error::Error;

use crate::storage::{
    CompareAndSetJobStateError, CompareAndSetPipelineStateError, CountIncomingEdgesError,
    CountJobsError, CreateJobError, CreatePipelineError, GetJobError, GetJobPayloadError,
    GetJobPipelineIdError, ListJobsError, ListNextJobsError, RegisterEdgesError, RemoveEdgesError,
};
use crate::{JobId, PipelineId};

#[derive(Debug, thiserror::Error)]
pub enum StartPipelineError<StorageError: Error + 'static, CallbackError: Error + 'static> {
    #[error("job {0} is registered more than once")]
    DuplicateJob(JobId),
    #[error("job {0} does not belong to the pipeline")]
    UnknownJob(JobId),
    #[error("edge from {from} to {to} is registered more than once")]
    DuplicateEdge { from: JobId, to: JobId },
    #[error("pipeline contains a cycle")]
    Cycle,
    #[error("pipeline {0} is already registered with different jobs, edges or payloads")]
    DefinitionContradicts(PipelineId),
    #[error("invalid pipeline payload: {0}")]
    Payload(#[source] serde_json::Error),
    #[error(transparent)]
    CreatePipeline(#[from] CreatePipelineError<StorageError>),
    #[error(transparent)]
    CreateJob(#[from] CreateJobError<StorageError>),
    #[error(transparent)]
    GetPayload(#[from] GetJobPayloadError<StorageError>),
    #[error(transparent)]
    GetPipelineId(#[from] GetJobPipelineIdError<StorageError>),
    #[error(transparent)]
    GetJob(#[from] GetJobError<StorageError>),
    #[error(transparent)]
    RegisterEdges(#[from] RegisterEdgesError<StorageError>),
    #[error(transparent)]
    ListNext(#[from] ListNextJobsError<StorageError>),
    #[error(transparent)]
    Count(#[from] CountJobsError<StorageError>),
    #[error(transparent)]
    JobState(#[from] CompareAndSetJobStateError<StorageError>),
    #[error(transparent)]
    PipelineState(#[from] CompareAndSetPipelineStateError<StorageError>),
    #[error(transparent)]
    Dispatch(#[from] DispatchJobError<StorageError, CallbackError>),
    #[error(transparent)]
    Storage(StorageError),
}

#[derive(Debug, thiserror::Error)]
pub enum CancelPipelineError<StorageError: Error + 'static> {
    #[error(transparent)]
    PipelineState(#[from] CompareAndSetPipelineStateError<StorageError>),
    #[error(transparent)]
    Storage(StorageError),
}

#[derive(Debug, thiserror::Error)]
pub enum JobFailedError<StorageError: Error + 'static> {
    #[error(transparent)]
    GetPipelineId(#[from] GetJobPipelineIdError<StorageError>),
    #[error(transparent)]
    PipelineState(#[from] CompareAndSetPipelineStateError<StorageError>),
    #[error(transparent)]
    JobState(#[from] CompareAndSetJobStateError<StorageError>),
    #[error(transparent)]
    Storage(StorageError),
}

#[derive(Debug, thiserror::Error)]
pub enum JobCompletedError<StorageError: Error + 'static, CallbackError: Error + 'static> {
    #[error(transparent)]
    GetPipelineId(#[from] GetJobPipelineIdError<StorageError>),
    #[error(transparent)]
    PipelineState(#[from] CompareAndSetPipelineStateError<StorageError>),
    #[error(transparent)]
    JobState(#[from] CompareAndSetJobStateError<StorageError>),
    #[error(transparent)]
    GetJob(#[from] GetJobError<StorageError>),
    #[error(transparent)]
    RemoveEdges(#[from] RemoveEdgesError<StorageError>),
    #[error(transparent)]
    CountIncoming(#[from] CountIncomingEdgesError<StorageError>),
    #[error(transparent)]
    Count(#[from] CountJobsError<StorageError>),
    #[error(transparent)]
    Dispatch(#[from] DispatchJobError<StorageError, CallbackError>),
    #[error(transparent)]
    Storage(StorageError),
}

#[derive(Debug, thiserror::Error)]
pub enum DispatchJobError<StorageError: Error + 'static, CallbackError: Error + 'static> {
    #[error(transparent)]
    ListJobs(#[from] ListJobsError<StorageError>),
    #[error(transparent)]
    GetPipelineId(#[from] GetJobPipelineIdError<StorageError>),
    #[error(transparent)]
    PipelineState(#[from] CompareAndSetPipelineStateError<StorageError>),
    #[error(transparent)]
    JobState(#[from] CompareAndSetJobStateError<StorageError>),
    #[error(transparent)]
    GetPayload(#[from] GetJobPayloadError<StorageError>),
    #[error("failed to read payload for job {job_id}: {source}")]
    Payload {
        job_id: JobId,
        #[source]
        source: serde_json::Error,
    },
    #[error("start callback for job {job_id} failed: {source}")]
    Callback {
        job_id: JobId,
        #[source]
        source: CallbackError,
    },
    #[error(transparent)]
    Storage(StorageError),
}

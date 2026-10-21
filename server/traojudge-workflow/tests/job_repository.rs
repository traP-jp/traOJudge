use serde_json::json;
use traojudge_workflow::storage::inmemory::InMemoryUnitOfWorkProvider;
use traojudge_workflow::storage::{
    CompareAndSetJobStateError, CreateJobError, GetJobError, GetJobPayloadError,
    GetJobPipelineIdError, JobRepository, PipelineRepository, UnitOfWork, UnitOfWorkProvider,
};
use traojudge_workflow::{JobId, JobState, PipelineId, PipelineState};

const STATES: [JobState; 7] = [
    JobState::Waiting,
    JobState::Ready,
    JobState::Starting,
    JobState::Running,
    JobState::Completed,
    JobState::Failed,
    JobState::Cancelled,
];

async fn job_in_state(state: JobState) -> (InMemoryUnitOfWorkProvider, JobId) {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let job_id = JobId::new();
    let mut work = provider.begin().await.unwrap();
    let pipeline_id = PipelineId::new();
    work.pipelines().create(pipeline_id).await.unwrap();
    work.jobs()
        .create(job_id, pipeline_id, serde_json::json!({}))
        .await
        .unwrap();
    if state != JobState::Waiting {
        work.jobs()
            .compare_and_set_state(job_id, JobState::Waiting, state)
            .await
            .unwrap();
    }
    work.commit().await.unwrap();
    (provider, job_id)
}

async fn read_state(
    provider: &InMemoryUnitOfWorkProvider,
    job_id: JobId,
) -> Result<JobState, GetJobError<sqlx::Error>> {
    let mut work = provider.begin().await.unwrap();
    let state = work.jobs().get(job_id).await;
    work.rollback().await.unwrap();
    state
}

#[test]
fn state_names_are_stable() {
    let names = [
        "waiting",
        "ready",
        "starting",
        "running",
        "completed",
        "failed",
        "cancelled",
    ];
    for (state, name) in STATES.into_iter().zip(names) {
        assert_eq!(serde_json::to_value(state).unwrap(), json!(name));
        assert_eq!(
            serde_json::from_value::<JobState>(json!(name)).unwrap(),
            state
        );
    }
}

#[tokio::test]
async fn newly_created_jobs_persist_in_waiting_state() {
    let (provider, job_id) = job_in_state(JobState::Waiting).await;

    assert_eq!(
        read_state(&provider, job_id).await.unwrap(),
        JobState::Waiting
    );
}

#[tokio::test]
async fn creation_persists_payload_and_duplicates_cannot_overwrite_it() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let pipeline_id = PipelineId::new();
    let job_id = JobId::new();
    let payload = json!({"revision": 2, "json": {"command": "compile"}});
    let mut work = provider.begin().await.unwrap();
    work.pipelines().create(pipeline_id).await.unwrap();
    work.jobs()
        .create(job_id, pipeline_id, payload.clone())
        .await
        .unwrap();
    assert!(matches!(
        work.jobs().create(job_id, pipeline_id, json!({"different": true})).await,
        Err(CreateJobError::AlreadyExists(existing)) if existing == job_id
    ));
    work.commit().await.unwrap();

    let mut work = provider.begin().await.unwrap();
    assert_eq!(work.jobs().get_payload(job_id).await.unwrap(), payload);
    assert!(matches!(
        work.jobs().get_payload(JobId::new()).await,
        Err(GetJobPayloadError::NotFound(_))
    ));
    work.rollback().await.unwrap();
}

#[tokio::test]
async fn job_pipeline_id_can_be_used_to_check_cancellation() {
    let (provider, job_id) = job_in_state(JobState::Running).await;
    let mut work = provider.begin().await.unwrap();
    let pipeline_id = work.jobs().get_pipeline_id(job_id).await.unwrap();
    assert_eq!(
        work.pipelines().get(pipeline_id).await.unwrap(),
        PipelineState::Running
    );
    work.pipelines()
        .compare_and_set_state(
            pipeline_id,
            PipelineState::Running,
            PipelineState::Cancelled,
        )
        .await
        .unwrap();
    work.commit().await.unwrap();

    let mut work = provider.begin().await.unwrap();
    let stored_pipeline_id = work.jobs().get_pipeline_id(job_id).await.unwrap();
    assert_eq!(stored_pipeline_id, pipeline_id);
    assert_eq!(
        work.pipelines().get(stored_pipeline_id).await.unwrap(),
        PipelineState::Cancelled
    );
    assert_eq!(work.jobs().get(job_id).await.unwrap(), JobState::Running);
    work.rollback().await.unwrap();
}

#[tokio::test]
async fn missing_jobs_return_a_pipeline_lookup_error() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let mut work = provider.begin().await.unwrap();
    let job_id = JobId::new();

    assert!(matches!(
        work.jobs().get_pipeline_id(job_id).await,
        Err(GetJobPipelineIdError::NotFound(missing)) if missing == job_id
    ));
    work.rollback().await.unwrap();
}

#[tokio::test]
async fn jobs_cannot_be_created_without_an_existing_pipeline() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let mut work = provider.begin().await.unwrap();
    let job_id = JobId::new();
    let pipeline_id = PipelineId::new();

    assert!(matches!(
        work.jobs().create(job_id, pipeline_id, serde_json::json!({})).await,
        Err(CreateJobError::PipelineNotFound(missing)) if missing == pipeline_id
    ));
    work.commit().await.unwrap();
    assert!(matches!(
        read_state(&provider, job_id).await,
        Err(GetJobError::NotFound(missing)) if missing == job_id
    ));
}

#[tokio::test]
async fn duplicate_creation_cannot_move_a_job_to_another_pipeline() {
    let (provider, job_id) = job_in_state(JobState::Ready).await;
    let mut work = provider.begin().await.unwrap();
    let original = work.jobs().get_pipeline_id(job_id).await.unwrap();
    let other = PipelineId::new();
    work.pipelines().create(other).await.unwrap();

    for pipeline_id in [other, PipelineId::new()] {
        assert!(matches!(
            work.jobs().create(job_id, pipeline_id, serde_json::json!({})).await,
            Err(CreateJobError::AlreadyExists(existing)) if existing == job_id
        ));
    }
    work.commit().await.unwrap();

    let mut work = provider.begin().await.unwrap();
    assert_eq!(work.jobs().get_pipeline_id(job_id).await.unwrap(), original);
    assert_eq!(work.jobs().get(job_id).await.unwrap(), JobState::Ready);
    work.rollback().await.unwrap();
}

#[tokio::test]
async fn missing_jobs_are_absent_and_cannot_be_updated() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let mut work = provider.begin().await.unwrap();
    let job_id = JobId::new();

    let state: Result<JobState, GetJobError<sqlx::Error>> = work.jobs().get(job_id).await;
    let updated: Result<(), CompareAndSetJobStateError<sqlx::Error>> = work
        .jobs()
        .compare_and_set_state(job_id, JobState::Waiting, JobState::Ready)
        .await;

    assert!(matches!(state, Err(GetJobError::NotFound(missing)) if missing == job_id));
    assert!(matches!(
        updated,
        Err(CompareAndSetJobStateError::NotFound(missing)) if missing == job_id
    ));
    work.commit().await.unwrap();
}

#[tokio::test]
async fn duplicate_creation_does_not_overwrite_existing_state() {
    let (provider, job_id) = job_in_state(JobState::Ready).await;
    let mut work = provider.begin().await.unwrap();
    let pipeline_id = work.jobs().get_pipeline_id(job_id).await.unwrap();

    assert!(matches!(
        work.jobs().create(job_id, pipeline_id, serde_json::json!({})).await,
        Err(CreateJobError::AlreadyExists(existing)) if existing == job_id
    ));
    work.commit().await.unwrap();

    assert_eq!(
        read_state(&provider, job_id).await.unwrap(),
        JobState::Ready
    );
}

#[tokio::test]
async fn repository_persists_states_without_deciding_transition_policy() {
    let (provider, job_id) = job_in_state(JobState::Waiting).await;
    let mut expected = JobState::Waiting;

    for next in STATES.into_iter().skip(1) {
        let mut work = provider.begin().await.unwrap();
        work.jobs()
            .compare_and_set_state(job_id, expected, next)
            .await
            .unwrap();
        work.commit().await.unwrap();

        assert_eq!(read_state(&provider, job_id).await.unwrap(), next);
        expected = next;
    }
}

#[tokio::test]
async fn stale_expected_state_does_not_overwrite_a_newer_state() {
    let (provider, job_id) = job_in_state(JobState::Ready).await;
    let mut work = provider.begin().await.unwrap();

    assert!(matches!(
        work.jobs()
            .compare_and_set_state(job_id, JobState::Waiting, JobState::Cancelled)
            .await,
        Err(CompareAndSetJobStateError::StateContradicts {
            job_id: conflicting,
            expected: JobState::Waiting,
            actual: JobState::Ready,
        }) if conflicting == job_id
    ));
    work.commit().await.unwrap();

    assert_eq!(
        read_state(&provider, job_id).await.unwrap(),
        JobState::Ready
    );
}

#[tokio::test]
async fn rollback_and_drop_discard_job_creation() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();

    for explicit_rollback in [true, false] {
        let job_id = JobId::new();
        let mut work = provider.begin().await.unwrap();
        let pipeline_id = PipelineId::new();
        work.pipelines().create(pipeline_id).await.unwrap();
        work.jobs()
            .create(job_id, pipeline_id, serde_json::json!({}))
            .await
            .unwrap();
        assert_eq!(work.jobs().get(job_id).await.unwrap(), JobState::Waiting);
        if explicit_rollback {
            work.rollback().await.unwrap();
        } else {
            drop(work);
        }

        assert!(matches!(
            read_state(&provider, job_id).await,
            Err(GetJobError::NotFound(missing)) if missing == job_id
        ));
        let mut work = provider.begin().await.unwrap();
        assert!(matches!(
            work.jobs().get_pipeline_id(job_id).await,
            Err(GetJobPipelineIdError::NotFound(missing)) if missing == job_id
        ));
        work.rollback().await.unwrap();
    }
}

#[tokio::test]
async fn rollback_and_drop_discard_state_changes() {
    let (provider, job_id) = job_in_state(JobState::Waiting).await;

    for explicit_rollback in [true, false] {
        let mut work = provider.begin().await.unwrap();
        work.jobs()
            .compare_and_set_state(job_id, JobState::Waiting, JobState::Ready)
            .await
            .unwrap();
        if explicit_rollback {
            work.rollback().await.unwrap();
        } else {
            drop(work);
        }

        assert_eq!(
            read_state(&provider, job_id).await.unwrap(),
            JobState::Waiting
        );
    }
}

#[tokio::test]
async fn concurrent_completion_notifications_accept_only_one_result() {
    let (provider, job_id) = job_in_state(JobState::Running).await;
    let completion_provider = provider.clone();
    let failure_provider = provider.clone();

    let completed = tokio::spawn(async move {
        let mut work = completion_provider.begin().await.unwrap();
        let updated = work
            .jobs()
            .compare_and_set_state(job_id, JobState::Running, JobState::Completed)
            .await;
        work.commit().await.unwrap();
        updated
    });
    let failed = tokio::spawn(async move {
        let mut work = failure_provider.begin().await.unwrap();
        let updated = work
            .jobs()
            .compare_and_set_state(job_id, JobState::Running, JobState::Failed)
            .await;
        work.commit().await.unwrap();
        updated
    });

    let completed = completed.await.unwrap();
    let failed = failed.await.unwrap();
    assert_ne!(completed.is_ok(), failed.is_ok());
    let expected = if completed.is_ok() {
        JobState::Completed
    } else {
        JobState::Failed
    };
    for error in [completed.err(), failed.err()].into_iter().flatten() {
        assert!(matches!(
            error,
            CompareAndSetJobStateError::StateContradicts {
                job_id: conflicting,
                expected: JobState::Running,
                actual,
            } if conflicting == job_id && actual == expected
        ));
    }
    assert_eq!(read_state(&provider, job_id).await.unwrap(), expected);
}

#[tokio::test]
async fn cancelled_jobs_ignore_updates_using_their_previous_state() {
    for current in [
        JobState::Waiting,
        JobState::Ready,
        JobState::Starting,
        JobState::Running,
    ] {
        let (provider, job_id) = job_in_state(current).await;
        let mut work = provider.begin().await.unwrap();
        work.jobs()
            .compare_and_set_state(job_id, current, JobState::Cancelled)
            .await
            .unwrap();
        assert!(matches!(
            work.jobs()
                .compare_and_set_state(job_id, current, JobState::Completed)
                .await,
            Err(CompareAndSetJobStateError::StateContradicts {
                job_id: conflicting,
                expected,
                actual: JobState::Cancelled,
            }) if conflicting == job_id && expected == current
        ));
        work.commit().await.unwrap();

        assert_eq!(
            read_state(&provider, job_id).await.unwrap(),
            JobState::Cancelled
        );
    }
}

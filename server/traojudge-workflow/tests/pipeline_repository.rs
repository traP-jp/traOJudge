use serde_json::json;
use traojudge_workflow::storage::inmemory::InMemoryUnitOfWorkProvider;
use traojudge_workflow::storage::{
    CompareAndSetPipelineStateError, CreatePipelineError, GetJobError, GetPipelineError,
    JobRepository, PipelineRepository, UnitOfWork, UnitOfWorkProvider,
};
use traojudge_workflow::{JobId, PipelineId, PipelineState};
use uuid::Uuid;

async fn running_pipeline() -> (InMemoryUnitOfWorkProvider, PipelineId) {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let pipeline_id = PipelineId::new();
    let mut work = provider.begin().await.unwrap();
    work.pipelines().create(pipeline_id).await.unwrap();
    work.commit().await.unwrap();
    (provider, pipeline_id)
}

async fn read_state(
    provider: &InMemoryUnitOfWorkProvider,
    pipeline_id: PipelineId,
) -> Result<PipelineState, GetPipelineError<sqlx::Error>> {
    let mut work = provider.begin().await.unwrap();
    let state = work.pipelines().get(pipeline_id).await;
    work.rollback().await.unwrap();
    state
}

#[test]
fn pipeline_id_round_trips_as_a_uuid_string() {
    let uuid = Uuid::from_u128(42);
    let pipeline_id = PipelineId::from(uuid);

    let serialized = serde_json::to_value(pipeline_id).unwrap();
    let restored: PipelineId = serde_json::from_value(serialized.clone()).unwrap();

    assert_eq!(serialized, json!(uuid.to_string()));
    assert_eq!(restored, pipeline_id);
    assert_eq!(restored.as_uuid(), &uuid);
    assert_eq!(restored.to_string(), uuid.to_string());
    assert_eq!(Uuid::from(restored), uuid);
}

#[test]
fn pipeline_state_names_are_stable() {
    for (state, name) in [
        (PipelineState::Running, "running"),
        (PipelineState::Completed, "completed"),
        (PipelineState::Cancelled, "cancelled"),
    ] {
        assert_eq!(serde_json::to_value(state).unwrap(), json!(name));
        assert_eq!(
            serde_json::from_value::<PipelineState>(json!(name)).unwrap(),
            state
        );
    }
}

#[tokio::test]
async fn newly_created_pipelines_persist_in_running_state() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let pipeline_id = PipelineId::new();
    let mut work = provider.begin().await.unwrap();
    let mut repository: Box<dyn PipelineRepository<Error = sqlx::Error> + '_> =
        Box::new(work.pipelines());
    repository.create(pipeline_id).await.unwrap();
    assert_eq!(
        repository.get(pipeline_id).await.unwrap(),
        PipelineState::Running
    );
    drop(repository);
    work.commit().await.unwrap();

    assert_eq!(
        read_state(&provider, pipeline_id).await.unwrap(),
        PipelineState::Running
    );
}

#[tokio::test]
async fn missing_pipelines_return_operation_specific_errors() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let pipeline_id = PipelineId::new();
    let mut work = provider.begin().await.unwrap();

    assert!(matches!(
        work.pipelines().get(pipeline_id).await,
        Err(GetPipelineError::NotFound(missing)) if missing == pipeline_id
    ));
    assert!(matches!(
        work.pipelines().compare_and_set_state(pipeline_id, PipelineState::Running, PipelineState::Completed).await,
        Err(CompareAndSetPipelineStateError::NotFound(missing)) if missing == pipeline_id
    ));
    work.rollback().await.unwrap();
}

#[tokio::test]
async fn pipelines_can_complete_or_be_cancelled() {
    for next in [PipelineState::Completed, PipelineState::Cancelled] {
        let (provider, pipeline_id) = running_pipeline().await;
        let mut work = provider.begin().await.unwrap();
        work.pipelines()
            .compare_and_set_state(pipeline_id, PipelineState::Running, next)
            .await
            .unwrap();
        work.commit().await.unwrap();

        assert_eq!(read_state(&provider, pipeline_id).await.unwrap(), next);
    }
}

#[tokio::test]
async fn duplicate_creation_preserves_the_existing_state() {
    for state in [PipelineState::Completed, PipelineState::Cancelled] {
        let (provider, pipeline_id) = running_pipeline().await;
        let mut work = provider.begin().await.unwrap();
        work.pipelines()
            .compare_and_set_state(pipeline_id, PipelineState::Running, state)
            .await
            .unwrap();

        assert!(matches!(
            work.pipelines().create(pipeline_id).await,
            Err(CreatePipelineError::AlreadyExists(existing)) if existing == pipeline_id
        ));
        work.commit().await.unwrap();

        assert_eq!(read_state(&provider, pipeline_id).await.unwrap(), state);
    }
}

#[tokio::test]
async fn state_mismatch_reports_expected_and_actual_without_overwriting() {
    let (provider, pipeline_id) = running_pipeline().await;
    let mut work = provider.begin().await.unwrap();
    work.pipelines()
        .compare_and_set_state(
            pipeline_id,
            PipelineState::Running,
            PipelineState::Completed,
        )
        .await
        .unwrap();

    assert!(matches!(
        work.pipelines().compare_and_set_state(pipeline_id, PipelineState::Running, PipelineState::Cancelled).await,
        Err(CompareAndSetPipelineStateError::StateContradicts {
            pipeline_id: conflicting,
            expected: PipelineState::Running,
            actual: PipelineState::Completed,
        }) if conflicting == pipeline_id
    ));
    work.commit().await.unwrap();

    assert_eq!(
        read_state(&provider, pipeline_id).await.unwrap(),
        PipelineState::Completed
    );
}

#[tokio::test]
async fn repository_does_not_decide_transition_policy() {
    let (provider, pipeline_id) = running_pipeline().await;
    let mut work = provider.begin().await.unwrap();
    work.pipelines()
        .compare_and_set_state(
            pipeline_id,
            PipelineState::Running,
            PipelineState::Completed,
        )
        .await
        .unwrap();
    work.pipelines()
        .compare_and_set_state(
            pipeline_id,
            PipelineState::Completed,
            PipelineState::Running,
        )
        .await
        .unwrap();
    work.commit().await.unwrap();

    assert_eq!(
        read_state(&provider, pipeline_id).await.unwrap(),
        PipelineState::Running
    );
}

#[tokio::test]
async fn rollback_and_drop_discard_pipeline_and_job_creation_together() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    for explicit_rollback in [true, false] {
        let pipeline_id = PipelineId::new();
        let job_id = JobId::from(*pipeline_id.as_uuid());
        let mut work = provider.begin().await.unwrap();
        work.pipelines().create(pipeline_id).await.unwrap();
        work.jobs()
            .create(job_id, pipeline_id, serde_json::json!({}))
            .await
            .unwrap();
        if explicit_rollback {
            work.rollback().await.unwrap();
        } else {
            drop(work);
        }

        let mut work = provider.begin().await.unwrap();
        assert!(
            matches!(work.pipelines().get(pipeline_id).await, Err(GetPipelineError::NotFound(missing)) if missing == pipeline_id)
        );
        assert!(
            matches!(work.jobs().get(job_id).await, Err(GetJobError::NotFound(missing)) if missing == job_id)
        );
        work.rollback().await.unwrap();
    }
}

#[tokio::test]
async fn rollback_and_drop_discard_state_changes() {
    let (provider, pipeline_id) = running_pipeline().await;
    for explicit_rollback in [true, false] {
        let mut work = provider.begin().await.unwrap();
        work.pipelines()
            .compare_and_set_state(
                pipeline_id,
                PipelineState::Running,
                PipelineState::Cancelled,
            )
            .await
            .unwrap();
        if explicit_rollback {
            work.rollback().await.unwrap();
        } else {
            drop(work);
        }

        assert_eq!(
            read_state(&provider, pipeline_id).await.unwrap(),
            PipelineState::Running
        );
    }
}

#[tokio::test]
async fn concurrent_completion_and_cancellation_accept_only_one_result() {
    let (provider, pipeline_id) = running_pipeline().await;
    let completion_provider = provider.clone();
    let cancellation_provider = provider.clone();
    let completed = tokio::spawn(async move {
        let mut work = completion_provider.begin().await.unwrap();
        let result = work
            .pipelines()
            .compare_and_set_state(
                pipeline_id,
                PipelineState::Running,
                PipelineState::Completed,
            )
            .await;
        work.commit().await.unwrap();
        result
    });
    let cancelled = tokio::spawn(async move {
        let mut work = cancellation_provider.begin().await.unwrap();
        let result = work
            .pipelines()
            .compare_and_set_state(
                pipeline_id,
                PipelineState::Running,
                PipelineState::Cancelled,
            )
            .await;
        work.commit().await.unwrap();
        result
    });

    let completed = completed.await.unwrap();
    let cancelled = cancelled.await.unwrap();
    assert_ne!(completed.is_ok(), cancelled.is_ok());
    let expected = if completed.is_ok() {
        PipelineState::Completed
    } else {
        PipelineState::Cancelled
    };
    for error in [completed.err(), cancelled.err()].into_iter().flatten() {
        assert!(
            matches!(error, CompareAndSetPipelineStateError::StateContradicts {
            pipeline_id: conflicting,
            expected: PipelineState::Running,
            actual,
        } if conflicting == pipeline_id && actual == expected)
        );
    }
    assert_eq!(read_state(&provider, pipeline_id).await.unwrap(), expected);
}

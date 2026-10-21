use traojudge_workflow::storage::inmemory::InMemoryUnitOfWorkProvider;
use traojudge_workflow::storage::{
    CountJobsError, JobRepository, PipelineRepository, UnitOfWork, UnitOfWorkProvider,
};
use traojudge_workflow::{JobId, JobState, PipelineId};

const STATES: [JobState; 7] = [
    JobState::Waiting,
    JobState::Ready,
    JobState::Starting,
    JobState::Running,
    JobState::Completed,
    JobState::Failed,
    JobState::Cancelled,
];

#[tokio::test]
async fn counts_only_requested_states_in_the_requested_pipeline() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let first = PipelineId::new();
    let second = PipelineId::new();
    let mut work = provider.begin().await.unwrap();
    for (pipeline_id, copies) in [(first, 1), (second, 2)] {
        work.pipelines().create(pipeline_id).await.unwrap();
        for state in STATES {
            for _ in 0..copies {
                let job_id = JobId::new();
                work.jobs()
                    .create(job_id, pipeline_id, serde_json::json!({}))
                    .await
                    .unwrap();
                work.jobs()
                    .compare_and_set_state(job_id, JobState::Waiting, state)
                    .await
                    .unwrap();
            }
        }
    }
    work.commit().await.unwrap();

    let mut work = provider.begin().await.unwrap();
    let mut repository: Box<dyn JobRepository<Error = sqlx::Error> + '_> = Box::new(work.jobs());
    for (pipeline_id, copies) in [(first, 1), (second, 2)] {
        for state in STATES {
            assert_eq!(
                repository.count(pipeline_id, &[state]).await.unwrap(),
                copies
            );
        }
        assert_eq!(
            repository.count(pipeline_id, &STATES).await.unwrap(),
            7 * copies
        );
        assert_eq!(
            repository
                .count(
                    pipeline_id,
                    &[JobState::Ready, JobState::Starting, JobState::Running]
                )
                .await
                .unwrap(),
            3 * copies
        );
        assert_eq!(
            repository
                .count(
                    pipeline_id,
                    &[JobState::Running, JobState::Running, JobState::Completed]
                )
                .await
                .unwrap(),
            2 * copies
        );
        assert_eq!(repository.count(pipeline_id, &[]).await.unwrap(), 0);
    }
    drop(repository);
    work.rollback().await.unwrap();
}

#[tokio::test]
async fn empty_pipelines_and_non_matching_states_return_zero() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let pipeline_id = PipelineId::new();
    let mut work = provider.begin().await.unwrap();
    work.pipelines().create(pipeline_id).await.unwrap();

    assert_eq!(work.jobs().count(pipeline_id, &STATES).await.unwrap(), 0);
    assert_eq!(work.jobs().count(pipeline_id, &[]).await.unwrap(), 0);

    work.jobs()
        .create(JobId::new(), pipeline_id, serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(
        work.jobs()
            .count(pipeline_id, &[JobState::Running])
            .await
            .unwrap(),
        0
    );
    work.rollback().await.unwrap();
}

#[tokio::test]
async fn missing_pipelines_return_an_operation_specific_error_even_with_no_states() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let pipeline_id = PipelineId::new();
    let mut work = provider.begin().await.unwrap();

    for states in [&STATES[..], &[]] {
        assert!(matches!(
            work.jobs().count(pipeline_id, states).await,
            Err(CountJobsError::PipelineNotFound(missing)) if missing == pipeline_id
        ));
    }
    work.rollback().await.unwrap();
}

#[tokio::test]
async fn counts_reflect_transactional_writes_and_their_rollback() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let pipeline_id = PipelineId::new();
    let job_id = JobId::new();
    let mut work = provider.begin().await.unwrap();
    work.pipelines().create(pipeline_id).await.unwrap();
    work.jobs()
        .create(job_id, pipeline_id, serde_json::json!({}))
        .await
        .unwrap();
    work.commit().await.unwrap();

    for explicit_rollback in [true, false] {
        let mut work = provider.begin().await.unwrap();
        work.jobs()
            .create(JobId::new(), pipeline_id, serde_json::json!({}))
            .await
            .unwrap();
        work.jobs()
            .compare_and_set_state(job_id, JobState::Waiting, JobState::Running)
            .await
            .unwrap();
        assert_eq!(work.jobs().count(pipeline_id, &STATES).await.unwrap(), 2);
        assert_eq!(
            work.jobs()
                .count(pipeline_id, &[JobState::Running])
                .await
                .unwrap(),
            1
        );
        if explicit_rollback {
            work.rollback().await.unwrap();
        } else {
            drop(work);
        }

        let mut work = provider.begin().await.unwrap();
        assert_eq!(work.jobs().count(pipeline_id, &STATES).await.unwrap(), 1);
        assert_eq!(
            work.jobs()
                .count(pipeline_id, &[JobState::Running])
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            work.jobs()
                .count(pipeline_id, &[JobState::Waiting])
                .await
                .unwrap(),
            1
        );
        work.rollback().await.unwrap();
    }
}

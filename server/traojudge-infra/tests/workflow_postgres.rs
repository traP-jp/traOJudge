//! PostgreSQL上でworkflow storageの契約を確認する結合テスト。
//!
//! `DATABASE_URL` に接続可能なPostgreSQLを指定し、`--ignored` を付けて実行する。
//! 各テストはsqlxが専用のデータベースを作成し、`postgres::MIGRATOR` を適用する。
//!
//! ```sh
//! DATABASE_URL=postgres://postgres:postgres@localhost:5432/traojudge \
//!     cargo test -p traojudge-infra --test workflow_postgres -- --ignored
//! ```

use std::collections::HashSet;

use serde_json::json;
use sqlx::PgPool;

use traojudge_infra::postgres::workflow::PostgresUnitOfWorkProvider;
use traojudge_workflow::storage::{
    CompareAndSetJobStateError, CompareAndSetPipelineStateError, CountIncomingEdgesError,
    CreateJobError, CreatePipelineError, EdgeRepository, GetJobError, GetJobPayloadError,
    GetJobPipelineIdError, GetPipelineError, JobRepository, ListJobsError, ListNextJobsError,
    PipelineRepository, RegisterEdgesError, RemoveEdgesError, UnitOfWork, UnitOfWorkProvider,
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

fn provider(pool: PgPool) -> PostgresUnitOfWorkProvider {
    PostgresUnitOfWorkProvider::new(pool)
}

async fn running_pipeline(pool: PgPool) -> (PostgresUnitOfWorkProvider, PipelineId) {
    let provider = provider(pool);
    let pipeline_id = PipelineId::new();
    let mut work = provider.begin().await.unwrap();
    work.pipelines().create(pipeline_id).await.unwrap();
    work.commit().await.unwrap();
    (provider, pipeline_id)
}

async fn job_in_state(pool: PgPool, state: JobState) -> (PostgresUnitOfWorkProvider, JobId) {
    let provider = provider(pool);
    let job_id = JobId::new();
    let mut work = provider.begin().await.unwrap();
    let pipeline_id = PipelineId::new();
    work.pipelines().create(pipeline_id).await.unwrap();
    work.jobs()
        .create(job_id, pipeline_id, json!({}))
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

async fn pipeline_state(
    provider: &PostgresUnitOfWorkProvider,
    pipeline_id: PipelineId,
) -> Result<PipelineState, GetPipelineError<sqlx::Error>> {
    let mut work = provider.begin().await.unwrap();
    let state = work.pipelines().get(pipeline_id).await;
    work.rollback().await.unwrap();
    state
}

async fn job_state(
    provider: &PostgresUnitOfWorkProvider,
    job_id: JobId,
) -> Result<JobState, GetJobError<sqlx::Error>> {
    let mut work = provider.begin().await.unwrap();
    let state = work.jobs().get(job_id).await;
    work.rollback().await.unwrap();
    state
}

#[ignore = "requires a PostgreSQL DATABASE_URL"]
#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
async fn pipeline_creation_get_and_state_transitions(pool: PgPool) {
    let (provider, pipeline_id) = running_pipeline(pool).await;
    assert_eq!(
        pipeline_state(&provider, pipeline_id).await.unwrap(),
        PipelineState::Running
    );

    for next in [PipelineState::Completed, PipelineState::Cancelled] {
        let (provider, pipeline_id) = running_pipeline(provider.pool().clone()).await;
        let mut work = provider.begin().await.unwrap();
        work.pipelines()
            .compare_and_set_state(pipeline_id, PipelineState::Running, next)
            .await
            .unwrap();
        work.commit().await.unwrap();
        assert_eq!(pipeline_state(&provider, pipeline_id).await.unwrap(), next);
    }
}

#[ignore = "requires a PostgreSQL DATABASE_URL"]
#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
async fn pipeline_duplicates_and_missing_pipelines(pool: PgPool) {
    let (provider, pipeline_id) = running_pipeline(pool).await;
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
        work.pipelines().create(pipeline_id).await,
        Err(CreatePipelineError::AlreadyExists(existing)) if existing == pipeline_id
    ));
    assert!(matches!(
        work.pipelines()
            .compare_and_set_state(pipeline_id, PipelineState::Running, PipelineState::Cancelled)
            .await,
        Err(CompareAndSetPipelineStateError::StateContradicts {
            pipeline_id: conflicting,
            expected: PipelineState::Running,
            actual: PipelineState::Completed,
        }) if conflicting == pipeline_id
    ));
    work.commit().await.unwrap();

    let missing = PipelineId::new();
    let mut work = provider.begin().await.unwrap();
    assert!(matches!(
        work.pipelines().get(missing).await,
        Err(GetPipelineError::NotFound(found)) if found == missing
    ));
    assert!(matches!(
        work.pipelines()
            .compare_and_set_state(missing, PipelineState::Running, PipelineState::Completed)
            .await,
        Err(CompareAndSetPipelineStateError::NotFound(found)) if found == missing
    ));
    work.rollback().await.unwrap();

    assert_eq!(
        pipeline_state(&provider, pipeline_id).await.unwrap(),
        PipelineState::Completed
    );
}

#[ignore = "requires a PostgreSQL DATABASE_URL"]
#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
async fn pipeline_rollback_and_drop_discard_changes(pool: PgPool) {
    let (provider, pipeline_id) = running_pipeline(pool).await;
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
            pipeline_state(&provider, pipeline_id).await.unwrap(),
            PipelineState::Running
        );
    }
}

#[ignore = "requires a PostgreSQL DATABASE_URL"]
#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
async fn concurrent_pipeline_transitions_accept_only_one_result(pool: PgPool) {
    let (provider, pipeline_id) = running_pipeline(pool).await;
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
    assert_eq!(
        pipeline_state(&provider, pipeline_id).await.unwrap(),
        expected
    );
}

#[ignore = "requires a PostgreSQL DATABASE_URL"]
#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
async fn jobs_persist_payloads_and_states(pool: PgPool) {
    let provider = provider(pool);
    let mut work = provider.begin().await.unwrap();
    let pipeline_id = PipelineId::new();
    work.pipelines().create(pipeline_id).await.unwrap();
    let job_id = JobId::new();
    let payload = json!({"revision": 2, "json": {"command": "compile"}});
    work.jobs()
        .create(job_id, pipeline_id, payload.clone())
        .await
        .unwrap();
    work.commit().await.unwrap();

    let mut work = provider.begin().await.unwrap();
    assert!(matches!(
        work.jobs().create(job_id, pipeline_id, json!({"different": true})).await,
        Err(CreateJobError::AlreadyExists(existing)) if existing == job_id
    ));
    assert_eq!(work.jobs().get(job_id).await.unwrap(), JobState::Waiting);
    assert_eq!(work.jobs().get_payload(job_id).await.unwrap(), payload);
    assert_eq!(
        work.jobs().get_pipeline_id(job_id).await.unwrap(),
        pipeline_id
    );
    work.rollback().await.unwrap();

    let mut work = provider.begin().await.unwrap();
    work.jobs()
        .compare_and_set_state(job_id, JobState::Waiting, JobState::Running)
        .await
        .unwrap();
    work.commit().await.unwrap();

    let other_job = JobId::new();
    let mut work = provider.begin().await.unwrap();
    work.jobs()
        .create(other_job, pipeline_id, json!({"revision": 1}))
        .await
        .unwrap();
    work.commit().await.unwrap();

    let mut work = provider.begin().await.unwrap();
    let states = work.jobs().get_many(&[job_id, other_job]).await;
    assert_eq!(states[0].as_ref().unwrap(), &JobState::Running);
    assert_eq!(states[1].as_ref().unwrap(), &JobState::Waiting);
    assert_eq!(work.jobs().get_payload(job_id).await.unwrap(), payload);
    work.rollback().await.unwrap();
}

#[ignore = "requires a PostgreSQL DATABASE_URL"]
#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
async fn jobs_cannot_be_created_without_a_pipeline(pool: PgPool) {
    let provider = provider(pool);
    let job_id = JobId::new();
    let pipeline_id = PipelineId::new();
    let mut work = provider.begin().await.unwrap();

    assert!(matches!(
        work.jobs().create(job_id, pipeline_id, json!({})).await,
        Err(CreateJobError::PipelineNotFound(missing)) if missing == pipeline_id
    ));
    work.commit().await.unwrap();

    assert!(matches!(
        job_state(&provider, job_id).await,
        Err(GetJobError::NotFound(missing)) if missing == job_id
    ));
}

#[ignore = "requires a PostgreSQL DATABASE_URL"]
#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
async fn job_lookup_and_payload_errors_are_operation_specific(pool: PgPool) {
    let provider = provider(pool);
    let missing = JobId::new();
    let mut work = provider.begin().await.unwrap();

    assert!(matches!(
        work.jobs().get(missing).await,
        Err(GetJobError::NotFound(found)) if found == missing
    ));
    assert!(matches!(
        work.jobs().get_pipeline_id(missing).await,
        Err(GetJobPipelineIdError::NotFound(found)) if found == missing
    ));
    assert!(matches!(
        work.jobs().get_payload(missing).await,
        Err(GetJobPayloadError::NotFound(found)) if found == missing
    ));
    assert!(matches!(
        work.jobs()
            .compare_and_set_state(missing, JobState::Waiting, JobState::Ready)
            .await,
        Err(CompareAndSetJobStateError::NotFound(found)) if found == missing
    ));
    work.rollback().await.unwrap();
}

#[ignore = "requires a PostgreSQL DATABASE_URL"]
#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
async fn job_counts_and_state_listing_are_scoped_to_the_pipeline(pool: PgPool) {
    let provider = provider(pool);
    let mut work = provider.begin().await.unwrap();
    let first_pipeline = PipelineId::new();
    let second_pipeline = PipelineId::new();
    work.pipelines().create(first_pipeline).await.unwrap();
    work.pipelines().create(second_pipeline).await.unwrap();

    let waiting = JobId::new();
    let running = JobId::new();
    let other_pipeline_job = JobId::new();
    work.jobs()
        .create(waiting, first_pipeline, json!({}))
        .await
        .unwrap();
    work.jobs()
        .create(running, first_pipeline, json!({}))
        .await
        .unwrap();
    work.jobs()
        .create(other_pipeline_job, second_pipeline, json!({}))
        .await
        .unwrap();
    work.jobs()
        .compare_and_set_state(running, JobState::Waiting, JobState::Running)
        .await
        .unwrap();
    work.commit().await.unwrap();

    let mut work = provider.begin().await.unwrap();
    assert_eq!(work.jobs().count(first_pipeline, &STATES).await.unwrap(), 2);
    assert_eq!(
        work.jobs()
            .count(first_pipeline, &[JobState::Running])
            .await
            .unwrap(),
        1
    );
    assert_eq!(work.jobs().count(first_pipeline, &[]).await.unwrap(), 0);
    assert_eq!(
        work.jobs()
            .list_by_state(first_pipeline, &[JobState::Waiting])
            .await
            .unwrap(),
        vec![waiting]
    );
    assert_eq!(
        work.jobs()
            .list_by_state(first_pipeline, &[JobState::Running])
            .await
            .unwrap(),
        vec![running]
    );
    assert!(matches!(
        work.jobs().count(PipelineId::new(), &STATES).await,
        Err(traojudge_workflow::storage::CountJobsError::PipelineNotFound(_))
    ));
    assert!(matches!(
        work.jobs().list_by_state(PipelineId::new(), &STATES).await,
        Err(ListJobsError::PipelineNotFound(_))
    ));
    work.rollback().await.unwrap();
}

#[ignore = "requires a PostgreSQL DATABASE_URL"]
#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
async fn create_many_reports_each_job_result(pool: PgPool) {
    let provider = provider(pool);
    let mut work = provider.begin().await.unwrap();
    let pipeline_id = PipelineId::new();
    work.pipelines().create(pipeline_id).await.unwrap();

    let first = JobId::new();
    let second = JobId::new();
    let missing_pipeline_job = JobId::new();
    let missing_pipeline = PipelineId::new();
    let results = work
        .jobs()
        .create_many(&[
            (first, pipeline_id, json!({"n": 1})),
            (first, pipeline_id, json!({"n": 2})),
            (second, pipeline_id, json!({"n": 3})),
            (missing_pipeline_job, missing_pipeline, json!({"n": 4})),
        ])
        .await;
    assert!(results[0].is_ok());
    assert!(matches!(
        results[1],
        Err(CreateJobError::AlreadyExists(existing)) if existing == first
    ));
    assert!(results[2].is_ok());
    assert!(matches!(
        results[3],
        Err(CreateJobError::PipelineNotFound(missing)) if missing == missing_pipeline
    ));
    assert_eq!(
        work.jobs().get_payload(first).await.unwrap(),
        json!({"n": 1})
    );
    work.commit().await.unwrap();
}

#[ignore = "requires a PostgreSQL DATABASE_URL"]
#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
async fn compare_and_set_state_many_reports_each_update(pool: PgPool) {
    let provider = provider(pool);
    let mut work = provider.begin().await.unwrap();
    let pipeline_id = PipelineId::new();
    work.pipelines().create(pipeline_id).await.unwrap();
    let first = JobId::new();
    let second = JobId::new();
    let missing = JobId::new();
    for job_id in [first, second] {
        work.jobs()
            .create(job_id, pipeline_id, json!({}))
            .await
            .unwrap();
    }
    work.commit().await.unwrap();

    let mut work = provider.begin().await.unwrap();
    let results = work
        .jobs()
        .compare_and_set_state_many(&[
            (first, JobState::Waiting, JobState::Ready),
            (second, JobState::Running, JobState::Ready),
            (missing, JobState::Waiting, JobState::Ready),
        ])
        .await;
    assert!(results[0].is_ok());
    assert!(matches!(
        results[1],
        Err(CompareAndSetJobStateError::StateContradicts {
            job_id: conflicting,
            expected: JobState::Running,
            actual: JobState::Waiting,
        }) if conflicting == second
    ));
    assert!(matches!(
        results[2],
        Err(CompareAndSetJobStateError::NotFound(found)) if found == missing
    ));
    work.commit().await.unwrap();

    assert_eq!(job_state(&provider, first).await.unwrap(), JobState::Ready);
    assert_eq!(
        job_state(&provider, second).await.unwrap(),
        JobState::Waiting
    );
}

#[ignore = "requires a PostgreSQL DATABASE_URL"]
#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
async fn job_state_rollback_and_drop_discard_changes(pool: PgPool) {
    let (provider, job_id) = job_in_state(pool, JobState::Waiting).await;
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
            job_state(&provider, job_id).await.unwrap(),
            JobState::Waiting
        );
    }
}

#[ignore = "requires a PostgreSQL DATABASE_URL"]
#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
async fn edges_are_registered_listed_and_removed(pool: PgPool) {
    let provider = provider(pool);
    let mut work = provider.begin().await.unwrap();
    let pipeline_id = PipelineId::new();
    work.pipelines().create(pipeline_id).await.unwrap();
    let [first, second, target, other] = std::array::from_fn(|_| JobId::new());
    for job_id in [first, second, target, other] {
        work.jobs()
            .create(job_id, pipeline_id, json!({}))
            .await
            .unwrap();
    }
    work.commit().await.unwrap();

    let mut work = provider.begin().await.unwrap();
    work.edges()
        .register(&[(first, target), (second, target), (first, other)])
        .await
        .unwrap();
    assert_eq!(work.edges().count_incoming(target).await.unwrap(), 2);
    let removed = work.edges().remove_from(first).await.unwrap();
    assert_eq!(
        removed.into_iter().collect::<HashSet<_>>(),
        HashSet::from([target, other])
    );
    assert_eq!(work.edges().count_incoming(target).await.unwrap(), 1);
    assert_eq!(work.edges().count_incoming(other).await.unwrap(), 0);
    assert_eq!(work.edges().list_next(second).await.unwrap(), vec![target]);
    assert!(work.edges().remove_from(first).await.unwrap().is_empty());
    assert_eq!(work.edges().count_incoming(first).await.unwrap(), 0);
    work.commit().await.unwrap();

    let mut work = provider.begin().await.unwrap();
    assert_eq!(work.edges().count_incoming(target).await.unwrap(), 1);
    work.rollback().await.unwrap();
}

#[ignore = "requires a PostgreSQL DATABASE_URL"]
#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
async fn duplicate_edge_registration_rolls_back_the_batch(pool: PgPool) {
    let provider = provider(pool);
    let mut work = provider.begin().await.unwrap();
    let pipeline_id = PipelineId::new();
    work.pipelines().create(pipeline_id).await.unwrap();
    let [source, target, extra] = std::array::from_fn(|_| JobId::new());
    for job_id in [source, target, extra] {
        work.jobs()
            .create(job_id, pipeline_id, json!({}))
            .await
            .unwrap();
    }
    work.commit().await.unwrap();

    let mut work = provider.begin().await.unwrap();
    work.edges().register(&[(source, target)]).await.unwrap();
    assert!(matches!(
        work.edges().register(&[(source, extra), (source, target)]).await,
        Err(RegisterEdgesError::AlreadyExists { from, to }) if from == source && to == target
    ));
    assert!(matches!(
        work.edges().register(&[(source, extra), (source, extra)]).await,
        Err(RegisterEdgesError::AlreadyExists { from, to }) if from == source && to == extra
    ));
    work.commit().await.unwrap();

    let mut work = provider.begin().await.unwrap();
    assert_eq!(work.edges().list_next(source).await.unwrap(), vec![target]);
    work.rollback().await.unwrap();
}

#[ignore = "requires a PostgreSQL DATABASE_URL"]
#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
async fn edge_lookup_errors_are_operation_specific(pool: PgPool) {
    let provider = provider(pool);
    let missing = JobId::new();
    let mut work = provider.begin().await.unwrap();

    assert!(matches!(
        work.edges().list_next(missing).await,
        Err(ListNextJobsError::JobNotFound(found)) if found == missing
    ));
    assert!(matches!(
        work.edges().remove_from(missing).await,
        Err(RemoveEdgesError::JobNotFound(found)) if found == missing
    ));
    assert!(matches!(
        work.edges().count_incoming(missing).await,
        Err(CountIncomingEdgesError::JobNotFound(found)) if found == missing
    ));
    work.rollback().await.unwrap();
}

#[ignore = "requires a PostgreSQL DATABASE_URL"]
#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
async fn edge_registration_does_not_commit_the_enclosing_unit_of_work(pool: PgPool) {
    let provider = provider(pool);
    let mut work = provider.begin().await.unwrap();
    let pipeline_id = PipelineId::new();
    work.pipelines().create(pipeline_id).await.unwrap();
    let [source, target] = std::array::from_fn(|_| JobId::new());
    for job_id in [source, target] {
        work.jobs()
            .create(job_id, pipeline_id, json!({}))
            .await
            .unwrap();
    }
    work.edges().register(&[(source, target)]).await.unwrap();
    work.rollback().await.unwrap();

    let mut work = provider.begin().await.unwrap();
    assert!(matches!(
        work.jobs().get(source).await,
        Err(GetJobError::NotFound(found)) if found == source
    ));
    assert!(matches!(
        work.edges().list_next(source).await,
        Err(ListNextJobsError::JobNotFound(found)) if found == source
    ));
    work.rollback().await.unwrap();
}

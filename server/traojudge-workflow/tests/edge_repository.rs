use std::collections::HashSet;

use traojudge_workflow::storage::inmemory::InMemoryUnitOfWorkProvider;
use traojudge_workflow::storage::{
    CountIncomingEdgesError, EdgeRepository, GetJobError, JobRepository, ListNextJobsError,
    PipelineRepository, RegisterEdgesError, RemoveEdgesError, UnitOfWork, UnitOfWorkProvider,
};
use traojudge_workflow::{JobId, PipelineId};

async fn provider_with_jobs<const COUNT: usize>() -> (InMemoryUnitOfWorkProvider, [JobId; COUNT]) {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let job_ids = std::array::from_fn(|_| JobId::new());
    let mut work = provider.begin().await.unwrap();
    let pipeline_id = PipelineId::new();
    work.pipelines().create(pipeline_id).await.unwrap();
    for &job_id in &job_ids {
        work.jobs()
            .create(job_id, pipeline_id, serde_json::json!({}))
            .await
            .unwrap();
    }
    work.commit().await.unwrap();
    (provider, job_ids)
}

async fn next_jobs(
    provider: &InMemoryUnitOfWorkProvider,
    job_id: JobId,
) -> Result<Vec<JobId>, ListNextJobsError<sqlx::Error>> {
    let mut work = provider.begin().await.unwrap();
    let next = work.edges().list_next(job_id).await;
    work.rollback().await.unwrap();
    next
}

#[tokio::test]
async fn removing_outgoing_edges_releases_only_their_incoming_dependencies() {
    let (provider, [first, second, target, other]) = provider_with_jobs().await;
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
    let missing = JobId::new();
    assert!(matches!(work.edges().count_incoming(missing).await,
        Err(CountIncomingEdgesError::JobNotFound(job_id)) if job_id == missing));
    assert!(matches!(work.edges().remove_from(missing).await,
        Err(RemoveEdgesError::JobNotFound(job_id)) if job_id == missing));
    work.rollback().await.unwrap();
}

#[tokio::test]
async fn rollback_and_drop_restore_removed_edges() {
    let (provider, [source, target]) = provider_with_jobs().await;
    let mut work = provider.begin().await.unwrap();
    work.edges().register(&[(source, target)]).await.unwrap();
    work.commit().await.unwrap();
    for explicit_rollback in [true, false] {
        let mut work = provider.begin().await.unwrap();
        assert_eq!(
            work.edges().remove_from(source).await.unwrap(),
            vec![target]
        );
        assert_eq!(work.edges().count_incoming(target).await.unwrap(), 0);
        if explicit_rollback {
            work.rollback().await.unwrap();
        } else {
            drop(work);
        }
        let mut work = provider.begin().await.unwrap();
        assert_eq!(work.edges().count_incoming(target).await.unwrap(), 1);
        assert_eq!(work.edges().list_next(source).await.unwrap(), vec![target]);
        work.rollback().await.unwrap();
    }
}

#[tokio::test]
async fn registers_many_to_many_relations() {
    let (provider, [first_source, second_source, first_target, second_target]) =
        provider_with_jobs().await;
    let mut work = provider.begin().await.unwrap();
    work.edges()
        .register(&[
            (first_source, first_target),
            (first_source, second_target),
            (second_source, first_target),
            (second_source, second_target),
        ])
        .await
        .unwrap();
    work.commit().await.unwrap();

    for source in [first_source, second_source] {
        let next = next_jobs(&provider, source).await.unwrap();
        assert_eq!(next.len(), 2);
        assert_eq!(
            next.into_iter().collect::<HashSet<_>>(),
            HashSet::from([first_target, second_target])
        );
    }
    for target in [first_target, second_target] {
        assert!(next_jobs(&provider, target).await.unwrap().is_empty());
    }
}

#[tokio::test]
async fn lists_only_direct_successors() {
    let (provider, [root, middle, leaf]) = provider_with_jobs().await;
    let mut work = provider.begin().await.unwrap();
    work.edges()
        .register(&[(root, middle), (middle, leaf)])
        .await
        .unwrap();
    work.commit().await.unwrap();

    assert_eq!(next_jobs(&provider, root).await.unwrap(), vec![middle]);
    assert_eq!(next_jobs(&provider, middle).await.unwrap(), vec![leaf]);
    assert!(next_jobs(&provider, leaf).await.unwrap().is_empty());
}

#[tokio::test]
async fn isolated_jobs_have_no_successors() {
    let (provider, [job_id]) = provider_with_jobs().await;

    assert!(next_jobs(&provider, job_id).await.unwrap().is_empty());
}

#[tokio::test]
async fn listing_a_missing_job_returns_an_operation_specific_error() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let job_id = JobId::new();

    assert!(matches!(
        next_jobs(&provider, job_id).await,
        Err(ListNextJobsError::JobNotFound(missing)) if missing == job_id
    ));
}

#[tokio::test]
async fn empty_registration_succeeds() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let mut work = provider.begin().await.unwrap();

    work.edges().register(&[]).await.unwrap();
    work.commit().await.unwrap();
}

#[tokio::test]
async fn missing_endpoints_do_not_leave_partially_registered_edges() {
    let (provider, [source, target]) = provider_with_jobs().await;

    for missing_is_source in [true, false] {
        let missing = JobId::new();
        let extra_job = JobId::new();
        let failing_edge = if missing_is_source {
            (missing, target)
        } else {
            (source, missing)
        };
        let mut work = provider.begin().await.unwrap();
        let pipeline_id = work.jobs().get_pipeline_id(source).await.unwrap();
        work.jobs()
            .create(extra_job, pipeline_id, serde_json::json!({}))
            .await
            .unwrap();

        assert!(matches!(
            work.edges().register(&[(source, target), failing_edge]).await,
            Err(RegisterEdgesError::JobNotFound(job_id)) if job_id == missing
        ));
        assert!(work.edges().list_next(source).await.unwrap().is_empty());
        work.commit().await.unwrap();

        assert!(next_jobs(&provider, source).await.unwrap().is_empty());
        assert!(next_jobs(&provider, extra_job).await.unwrap().is_empty());
    }
}

#[tokio::test]
async fn duplicate_edges_within_a_batch_roll_back_the_batch() {
    let (provider, [source, target]) = provider_with_jobs().await;
    let mut work = provider.begin().await.unwrap();

    assert!(matches!(
        work.edges().register(&[(source, target), (source, target)]).await,
        Err(RegisterEdgesError::AlreadyExists { from, to }) if from == source && to == target
    ));
    work.commit().await.unwrap();

    assert!(next_jobs(&provider, source).await.unwrap().is_empty());
}

#[tokio::test]
async fn duplicate_registration_preserves_existing_edges_and_discards_new_ones() {
    let (provider, [source, existing_target, new_target]) = provider_with_jobs().await;
    let mut work = provider.begin().await.unwrap();
    work.edges()
        .register(&[(source, existing_target)])
        .await
        .unwrap();
    work.commit().await.unwrap();

    let mut work = provider.begin().await.unwrap();
    assert!(matches!(
        work.edges().register(&[(source, new_target), (source, existing_target)]).await,
        Err(RegisterEdgesError::AlreadyExists { from, to })
            if from == source && to == existing_target
    ));
    work.commit().await.unwrap();

    assert_eq!(
        next_jobs(&provider, source).await.unwrap(),
        vec![existing_target]
    );
}

#[tokio::test]
async fn rollback_and_drop_discard_registered_edges() {
    let (provider, [source, target]) = provider_with_jobs().await;

    for explicit_rollback in [true, false] {
        let mut work = provider.begin().await.unwrap();
        work.edges().register(&[(source, target)]).await.unwrap();
        assert_eq!(work.edges().list_next(source).await.unwrap(), vec![target]);
        if explicit_rollback {
            work.rollback().await.unwrap();
        } else {
            drop(work);
        }

        assert!(next_jobs(&provider, source).await.unwrap().is_empty());
    }
}

#[tokio::test]
async fn jobs_and_edges_are_created_in_the_same_unit_of_work() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let source = JobId::new();
    let target = JobId::new();
    let mut work = provider.begin().await.unwrap();
    let pipeline_id = PipelineId::new();
    work.pipelines().create(pipeline_id).await.unwrap();
    work.jobs()
        .create(source, pipeline_id, serde_json::json!({}))
        .await
        .unwrap();
    work.jobs()
        .create(target, pipeline_id, serde_json::json!({}))
        .await
        .unwrap();
    work.edges().register(&[(source, target)]).await.unwrap();
    assert_eq!(work.edges().list_next(source).await.unwrap(), vec![target]);
    work.commit().await.unwrap();

    assert_eq!(next_jobs(&provider, source).await.unwrap(), vec![target]);
}

#[tokio::test]
async fn edge_registration_does_not_commit_the_enclosing_unit_of_work() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let source = JobId::new();
    let target = JobId::new();
    let mut work = provider.begin().await.unwrap();
    let pipeline_id = PipelineId::new();
    work.pipelines().create(pipeline_id).await.unwrap();
    work.jobs()
        .create(source, pipeline_id, serde_json::json!({}))
        .await
        .unwrap();
    work.jobs()
        .create(target, pipeline_id, serde_json::json!({}))
        .await
        .unwrap();
    work.edges().register(&[(source, target)]).await.unwrap();
    work.rollback().await.unwrap();

    let mut work = provider.begin().await.unwrap();
    assert!(matches!(
        work.jobs().get(source).await,
        Err(GetJobError::NotFound(job_id)) if job_id == source
    ));
    assert!(matches!(
        work.edges().list_next(source).await,
        Err(ListNextJobsError::JobNotFound(job_id)) if job_id == source
    ));
    work.rollback().await.unwrap();
}

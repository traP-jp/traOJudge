use traojudge_workflow::storage::inmemory::InMemoryUnitOfWorkProvider;
use traojudge_workflow::storage::{
    CompareAndSetJobStateError, CreateJobError, GetJobError, JobRepository, PipelineRepository,
    UnitOfWork, UnitOfWorkProvider,
};
use traojudge_workflow::{JobId, JobState, PipelineId};

#[tokio::test]
async fn create_many_reports_each_result_and_continues_after_duplicates() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let first = JobId::new();
    let second = JobId::new();
    let mut work = provider.begin().await.unwrap();
    let pipeline_id = PipelineId::new();
    work.pipelines().create(pipeline_id).await.unwrap();
    let mut repository: Box<dyn JobRepository<Error = sqlx::Error> + '_> = Box::new(work.jobs());

    let results = repository
        .create_many(&[
            (first, pipeline_id, serde_json::json!({})),
            (first, pipeline_id, serde_json::json!({})),
            (second, pipeline_id, serde_json::json!({})),
        ])
        .await;

    assert_eq!(results.len(), 3);
    assert!(results[0].is_ok());
    assert!(matches!(results[1], Err(CreateJobError::AlreadyExists(job_id)) if job_id == first));
    assert!(results[2].is_ok());
    drop(repository);
    work.commit().await.unwrap();

    let mut work = provider.begin().await.unwrap();
    for result in work.jobs().get_many(&[first, second]).await {
        assert_eq!(result.unwrap(), JobState::Waiting);
    }
    work.rollback().await.unwrap();
}

#[tokio::test]
async fn create_many_keeps_each_payload_and_does_not_overwrite_duplicates() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let pipeline_id = PipelineId::new();
    let first = JobId::new();
    let second = JobId::new();
    let mut work = provider.begin().await.unwrap();
    work.pipelines().create(pipeline_id).await.unwrap();
    let results = work
        .jobs()
        .create_many(&[
            (first, pipeline_id, serde_json::json!({"input": "first"})),
            (
                first,
                pipeline_id,
                serde_json::json!({"input": "duplicate"}),
            ),
            (second, pipeline_id, serde_json::json!({"input": "second"})),
        ])
        .await;
    assert!(results[0].is_ok());
    assert!(matches!(results[1], Err(CreateJobError::AlreadyExists(job_id)) if job_id == first));
    assert!(results[2].is_ok());
    work.commit().await.unwrap();

    let mut work = provider.begin().await.unwrap();
    assert_eq!(
        work.jobs().get_payload(first).await.unwrap(),
        serde_json::json!({"input": "first"})
    );
    assert_eq!(
        work.jobs().get_payload(second).await.unwrap(),
        serde_json::json!({"input": "second"})
    );
    work.rollback().await.unwrap();
}

#[tokio::test]
async fn create_many_preserves_pipeline_membership_and_continues_after_missing_pipelines() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let mut work = provider.begin().await.unwrap();
    let first_pipeline = PipelineId::new();
    let second_pipeline = PipelineId::new();
    let missing_pipeline = PipelineId::new();
    work.pipelines().create(first_pipeline).await.unwrap();
    work.pipelines().create(second_pipeline).await.unwrap();
    let first = JobId::new();
    let second = JobId::new();
    let third = JobId::new();

    let results = work
        .jobs()
        .create_many(&[
            (first, missing_pipeline, serde_json::json!({})),
            (first, first_pipeline, serde_json::json!({})),
            (first, second_pipeline, serde_json::json!({})),
            (second, second_pipeline, serde_json::json!({})),
            (third, missing_pipeline, serde_json::json!({})),
            (second, missing_pipeline, serde_json::json!({})),
            (third, second_pipeline, serde_json::json!({})),
        ])
        .await;

    assert_eq!(results.len(), 7);
    assert!(matches!(
        results[0], Err(CreateJobError::PipelineNotFound(pipeline_id)) if pipeline_id == missing_pipeline
    ));
    assert!(results[1].is_ok());
    assert!(matches!(results[2], Err(CreateJobError::AlreadyExists(job_id)) if job_id == first));
    assert!(results[3].is_ok());
    assert!(matches!(
        results[4], Err(CreateJobError::PipelineNotFound(pipeline_id)) if pipeline_id == missing_pipeline
    ));
    assert!(matches!(results[5], Err(CreateJobError::AlreadyExists(job_id)) if job_id == second));
    assert!(results[6].is_ok());
    work.commit().await.unwrap();

    let mut work = provider.begin().await.unwrap();
    for (job_id, expected) in [
        (first, first_pipeline),
        (second, second_pipeline),
        (third, second_pipeline),
    ] {
        assert_eq!(work.jobs().get_pipeline_id(job_id).await.unwrap(), expected);
        assert_eq!(work.jobs().get(job_id).await.unwrap(), JobState::Waiting);
    }
    work.rollback().await.unwrap();
}

#[tokio::test]
async fn get_many_preserves_order_duplicates_and_missing_job_errors() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let first = JobId::new();
    let second = JobId::new();
    let missing = JobId::new();
    let mut work = provider.begin().await.unwrap();
    let pipeline_id = PipelineId::new();
    work.pipelines().create(pipeline_id).await.unwrap();
    work.jobs()
        .create(first, pipeline_id, serde_json::json!({}))
        .await
        .unwrap();
    work.jobs()
        .create(second, pipeline_id, serde_json::json!({}))
        .await
        .unwrap();
    work.jobs()
        .compare_and_set_state(second, JobState::Waiting, JobState::Ready)
        .await
        .unwrap();

    let results = work
        .jobs()
        .get_many(&[second, missing, first, second])
        .await;

    assert_eq!(results.len(), 4);
    assert!(matches!(results[0], Ok(JobState::Ready)));
    assert!(matches!(results[1], Err(GetJobError::NotFound(job_id)) if job_id == missing));
    assert!(matches!(results[2], Ok(JobState::Waiting)));
    assert!(matches!(results[3], Ok(JobState::Ready)));
    work.rollback().await.unwrap();
}

#[tokio::test]
async fn state_updates_preserve_order_and_continue_after_each_kind_of_failure() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let first = JobId::new();
    let second = JobId::new();
    let third = JobId::new();
    let missing = JobId::new();
    let mut work = provider.begin().await.unwrap();
    let pipeline_id = PipelineId::new();
    work.pipelines().create(pipeline_id).await.unwrap();
    for result in work
        .jobs()
        .create_many(&[
            (first, pipeline_id, serde_json::json!({})),
            (second, pipeline_id, serde_json::json!({})),
            (third, pipeline_id, serde_json::json!({})),
        ])
        .await
    {
        result.unwrap();
    }

    let results = work
        .jobs()
        .compare_and_set_state_many(&[
            (first, JobState::Waiting, JobState::Ready),
            (second, JobState::Ready, JobState::Completed),
            (missing, JobState::Waiting, JobState::Starting),
            (third, JobState::Waiting, JobState::Cancelled),
            (first, JobState::Ready, JobState::Starting),
        ])
        .await;

    assert_eq!(results.len(), 5);
    assert!(results[0].is_ok());
    assert!(
        matches!(results[1], Err(CompareAndSetJobStateError::StateContradicts {
        job_id,
        expected: JobState::Ready,
        actual: JobState::Waiting,
    }) if job_id == second)
    );
    assert!(
        matches!(results[2], Err(CompareAndSetJobStateError::NotFound(job_id)) if job_id == missing)
    );
    assert!(results[3].is_ok());
    assert!(results[4].is_ok());
    work.commit().await.unwrap();

    let mut work = provider.begin().await.unwrap();
    let states: Vec<_> = work
        .jobs()
        .get_many(&[first, second, third])
        .await
        .into_iter()
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        states,
        [JobState::Starting, JobState::Waiting, JobState::Cancelled]
    );
    work.rollback().await.unwrap();
}

#[tokio::test]
async fn failed_update_can_be_followed_by_a_successful_update_of_the_same_job() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let job_id = JobId::new();
    let mut work = provider.begin().await.unwrap();
    let pipeline_id = PipelineId::new();
    work.pipelines().create(pipeline_id).await.unwrap();
    work.jobs()
        .create(job_id, pipeline_id, serde_json::json!({}))
        .await
        .unwrap();

    let results = work
        .jobs()
        .compare_and_set_state_many(&[
            (job_id, JobState::Ready, JobState::Running),
            (job_id, JobState::Waiting, JobState::Ready),
            (job_id, JobState::Ready, JobState::Starting),
        ])
        .await;

    assert!(matches!(
        results[0],
        Err(CompareAndSetJobStateError::StateContradicts { .. })
    ));
    assert!(results[1].is_ok());
    assert!(results[2].is_ok());
    assert_eq!(work.jobs().get(job_id).await.unwrap(), JobState::Starting);
    work.rollback().await.unwrap();
}

#[tokio::test]
async fn empty_requests_return_empty_results() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let mut work = provider.begin().await.unwrap();

    assert!(work.jobs().create_many(&[]).await.is_empty());
    assert!(work.jobs().get_many(&[]).await.is_empty());
    assert!(work.jobs().compare_and_set_state_many(&[]).await.is_empty());
    work.commit().await.unwrap();
}

#[tokio::test]
async fn rollback_and_drop_control_successful_many_writes() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let existing = JobId::new();
    let mut work = provider.begin().await.unwrap();
    let pipeline_id = PipelineId::new();
    work.pipelines().create(pipeline_id).await.unwrap();
    work.jobs()
        .create(existing, pipeline_id, serde_json::json!({}))
        .await
        .unwrap();
    work.commit().await.unwrap();

    for explicit_rollback in [true, false] {
        let created = JobId::new();
        let mut work = provider.begin().await.unwrap();
        for result in work
            .jobs()
            .create_many(&[(created, pipeline_id, serde_json::json!({}))])
            .await
        {
            result.unwrap();
        }
        for result in work
            .jobs()
            .compare_and_set_state_many(&[
                (existing, JobState::Waiting, JobState::Ready),
                (created, JobState::Waiting, JobState::Running),
            ])
            .await
        {
            result.unwrap();
        }
        if explicit_rollback {
            work.rollback().await.unwrap();
        } else {
            drop(work);
        }

        let mut work = provider.begin().await.unwrap();
        let results = work.jobs().get_many(&[existing, created]).await;
        assert!(matches!(results[0], Ok(JobState::Waiting)));
        assert!(matches!(results[1], Err(GetJobError::NotFound(job_id)) if job_id == created));
        work.rollback().await.unwrap();
    }
}

#[tokio::test]
async fn large_requests_cross_batch_boundaries_and_keep_per_item_results() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let job_ids: Vec<_> = (0..1000).map(|_| JobId::new()).collect();
    let mut requested = job_ids.clone();
    requested.insert(256, job_ids[0]);
    let mut work = provider.begin().await.unwrap();
    let pipeline_id = PipelineId::new();
    work.pipelines().create(pipeline_id).await.unwrap();

    let jobs: Vec<_> = requested
        .iter()
        .map(|&job_id| (job_id, pipeline_id, serde_json::json!({})))
        .collect();
    let results = work.jobs().create_many(&jobs).await;
    assert_eq!(results.len(), requested.len());
    for (index, result) in results.into_iter().enumerate() {
        if index == 256 {
            assert!(
                matches!(result, Err(CreateJobError::AlreadyExists(job_id)) if job_id == job_ids[0])
            );
        } else {
            result.unwrap();
        }
    }
    let updates: Vec<_> = job_ids
        .iter()
        .enumerate()
        .map(|(index, &job_id)| {
            let next = if index % 2 == 0 {
                JobState::Ready
            } else {
                JobState::Running
            };
            (job_id, JobState::Waiting, next)
        })
        .collect();
    for result in work.jobs().compare_and_set_state_many(&updates).await {
        result.unwrap();
    }
    requested.reverse();
    let results = work.jobs().get_many(&requested).await;
    assert_eq!(results.len(), requested.len());
    for (job_id, result) in requested.iter().zip(results) {
        let index = job_ids
            .iter()
            .position(|candidate| candidate == job_id)
            .unwrap();
        let expected = if index % 2 == 0 {
            JobState::Ready
        } else {
            JobState::Running
        };
        assert_eq!(result.unwrap(), expected);
    }
    work.commit().await.unwrap();
}

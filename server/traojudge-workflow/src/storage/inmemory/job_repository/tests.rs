use sqlx::Connection;

use super::*;

#[tokio::test]
async fn payload_column_rejects_sql_null_and_omitted_values() {
    let mut connection = SqliteConnection::connect("sqlite::memory:").await.unwrap();
    sqlx::query(super::super::pipeline_repository::CREATE_PIPELINES_TABLE)
        .execute(&mut connection)
        .await
        .unwrap();
    sqlx::query(CREATE_JOBS_TABLE)
        .execute(&mut connection)
        .await
        .unwrap();
    let pipeline_id = PipelineId::new();
    sqlx::query("INSERT INTO workflow_pipelines (id, state) VALUES (?, 'running')")
        .bind(pipeline_id.to_string())
        .execute(&mut connection)
        .await
        .unwrap();

    for query in [
        "INSERT INTO workflow_jobs (id, pipeline_id, state) VALUES (?, ?, 'waiting')",
        "INSERT INTO workflow_jobs (id, pipeline_id, payload, state) VALUES (?, ?, NULL, 'waiting')",
    ] {
        let error = sqlx::query(query)
            .bind(JobId::new().to_string())
            .bind(pipeline_id.to_string())
            .execute(&mut connection)
            .await
            .unwrap_err();
        assert!(error.as_database_error().is_some());
    }
}

#[tokio::test]
async fn batch_storage_failures_return_a_result_for_every_item_and_preserve_the_cause() {
    let mut connection = SqliteConnection::connect("sqlite::memory:").await.unwrap();
    let mut repository = InMemoryJobRepository::new(&mut connection);
    let job_ids: Vec<_> = (0..JOB_BATCH_SIZE + 1).map(|_| JobId::new()).collect();

    let pipeline_id = PipelineId::new();
    let jobs: Vec<_> = job_ids
        .iter()
        .map(|&job_id| (job_id, pipeline_id, serde_json::json!({})))
        .collect();
    let created = repository.create_many(&jobs).await;
    assert_eq!(created.len(), job_ids.len());
    for result in &created {
        assert!(
            matches!(result, Err(CreateJobError::Storage(error)) if error.as_database_error().is_some())
        );
    }
    if let (Err(CreateJobError::Storage(first)), Err(CreateJobError::Storage(second))) =
        (&created[0], &created[1])
    {
        assert!(Arc::ptr_eq(first, second));
    }

    let states = repository.get_many(&job_ids).await;
    assert_eq!(states.len(), job_ids.len());
    for result in states {
        assert!(
            matches!(result, Err(GetJobError::Storage(error)) if error.as_database_error().is_some())
        );
    }

    assert!(matches!(
        repository.get_pipeline_id(job_ids[0]).await,
        Err(GetJobPipelineIdError::Storage(error)) if error.as_database_error().is_some()
    ));

    for states in [&[JobState::Running][..], &[]] {
        assert!(matches!(
            repository.count(pipeline_id, states).await,
            Err(CountJobsError::Storage(error)) if error.as_database_error().is_some()
        ));
    }

    let updates: Vec<_> = job_ids
        .iter()
        .map(|&job_id| (job_id, JobState::Waiting, JobState::Ready))
        .collect();
    let updated = repository.compare_and_set_state_many(&updates).await;
    assert_eq!(updated.len(), job_ids.len());
    for result in updated {
        assert!(
            matches!(result, Err(CompareAndSetJobStateError::Storage(error)) if error.as_database_error().is_some())
        );
    }
}

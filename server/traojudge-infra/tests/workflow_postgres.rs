use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use traojudge_infra::postgres::{self, workflow::PostgresUnitOfWorkProvider};
use traojudge_workflow::{
    Engine, JobId, JobStartCallback, JobState, PipelineId, PipelineState,
    payload::{Payload, Revision0},
    storage::{JobRepository, PipelineRepository, UnitOfWork, UnitOfWorkProvider},
};
#[derive(Clone, Default)]
struct Started(Arc<Mutex<Vec<JobId>>>);
#[async_trait]
impl JobStartCallback for Started {
    type Input = Revision0<String>;
    type Error = std::io::Error;
    async fn start(&self, id: JobId, _payload: Payload<Self::Input>) -> std::io::Result<()> {
        self.0.lock().unwrap().push(id);
        Ok(())
    }
}
#[tokio::test]
#[ignore = "requires a disposable PostgreSQL database in DATABASE_URL"]
async fn postgres_survives_restart_and_serializes_parent_completion() {
    let url = std::env::var("DATABASE_URL").unwrap();
    let pool = postgres::connect(&url).await.unwrap();
    postgres::MIGRATOR.run(&pool).await.unwrap();
    let provider = PostgresUnitOfWorkProvider::new(pool);
    let callback = Started::default();
    let engine = Engine::new(provider.clone(), callback.clone());
    let pipeline = PipelineId::new();
    let jobs = [JobId::new(), JobId::new(), JobId::new(), JobId::new()];
    let payloads = || {
        jobs.iter()
            .map(|&id| (id, Payload::new(id.to_string()).unwrap()))
            .collect()
    };
    let edges = [
        (jobs[0], jobs[1]),
        (jobs[0], jobs[2]),
        (jobs[1], jobs[3]),
        (jobs[2], jobs[3]),
    ];
    engine
        .start_pipeline(pipeline, payloads(), &edges)
        .await
        .unwrap();
    engine.job_completed(jobs[0]).await.unwrap();
    // A fresh connection pool reads committed state and the original stored payloads.
    let provider2 = PostgresUnitOfWorkProvider::new(postgres::connect(&url).await.unwrap());
    let restarted = Engine::new(provider2.clone(), callback.clone());
    let (left, right) = tokio::join!(
        restarted.job_completed(jobs[1]),
        restarted.job_completed(jobs[2])
    );
    left.unwrap();
    right.unwrap();
    restarted.job_completed(jobs[2]).await.unwrap();
    restarted
        .start_pipeline(pipeline, payloads(), &edges)
        .await
        .unwrap();
    assert_eq!(
        callback
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|&&id| id == jobs[3])
            .count(),
        1
    );
    restarted.job_completed(jobs[3]).await.unwrap();
    let mut tx = provider2.begin().await.unwrap();
    assert_eq!(
        tx.pipelines().get(pipeline).await.unwrap(),
        PipelineState::Completed
    );
    assert_eq!(
        tx.jobs()
            .get_many(&jobs)
            .await
            .into_iter()
            .map(Result::unwrap)
            .collect::<Vec<_>>(),
        vec![JobState::Completed; 4]
    );
    tx.rollback().await.unwrap();
    let discarded = PipelineId::new();
    let mut tx = provider2.begin().await.unwrap();
    tx.pipelines().create(discarded).await.unwrap();
    drop(tx);
    let mut tx = provider2.begin().await.unwrap();
    assert!(tx.pipelines().get(discarded).await.is_err());
    tx.rollback().await.unwrap();
}

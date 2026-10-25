use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use traojudge_workflow::payload::{Payload, Revision, Revision0, Upcast};
use traojudge_workflow::storage::inmemory::InMemoryUnitOfWorkProvider;
use traojudge_workflow::storage::{
    EdgeRepository, GetPipelineError, JobRepository, PipelineRepository, UnitOfWork,
    UnitOfWorkProvider,
};
use traojudge_workflow::{Engine, JobId, JobStartCallback, JobState, PipelineId, PipelineState};

#[derive(Clone)]
struct RecordingCallback {
    provider: InMemoryUnitOfWorkProvider,
    calls: Arc<Mutex<Vec<(JobId, String)>>>,
    fail: Arc<Mutex<Option<JobId>>>,
}

impl RecordingCallback {
    fn new(provider: &InMemoryUnitOfWorkProvider) -> Self {
        Self {
            provider: provider.clone(),
            calls: Arc::default(),
            fail: Arc::default(),
        }
    }

    fn count(&self, job_id: JobId) -> usize {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|(id, _)| *id == job_id)
            .count()
    }
}

#[async_trait]
impl JobStartCallback for RecordingCallback {
    type Input = Revision0<String>;
    type Error = io::Error;

    async fn start(&self, job_id: JobId, payload: Payload<Self::Input>) -> Result<(), Self::Error> {
        let mut work = tokio::time::timeout(Duration::from_secs(2), self.provider.begin())
            .await
            .map_err(io::Error::other)?
            .map_err(io::Error::other)?;
        let state = work.jobs().get(job_id).await.map_err(io::Error::other)?;
        assert!(matches!(
            state,
            JobState::Starting | JobState::Running | JobState::Completed
        ));
        work.rollback().await.map_err(io::Error::other)?;
        self.calls
            .lock()
            .unwrap()
            .push((job_id, payload.into_value()));
        if *self.fail.lock().unwrap() == Some(job_id) {
            return Err(io::Error::other("dispatch failed"));
        }
        Ok(())
    }
}

fn jobs(job_ids: &[JobId]) -> Vec<(JobId, Payload<Revision0<String>>)> {
    job_ids
        .iter()
        .map(|&job_id| (job_id, Payload::new(job_id.to_string()).unwrap()))
        .collect()
}

async fn job_state(provider: &InMemoryUnitOfWorkProvider, job_id: JobId) -> JobState {
    let mut work = provider.begin().await.unwrap();
    let state = work.jobs().get(job_id).await.unwrap();
    work.rollback().await.unwrap();
    state
}

async fn pipeline_state(
    provider: &InMemoryUnitOfWorkProvider,
    pipeline_id: PipelineId,
) -> PipelineState {
    let mut work = provider.begin().await.unwrap();
    let state = work.pipelines().get(pipeline_id).await.unwrap();
    work.rollback().await.unwrap();
    state
}

#[tokio::test]
async fn diamond_pipeline_consumes_edges_and_waits_for_every_parent() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let callback = RecordingCallback::new(&provider);
    let engine = Engine::new(provider.clone(), callback.clone());
    let pipeline_id = PipelineId::new();
    let [root, left, right, join] = std::array::from_fn(|_| JobId::new());
    engine
        .start_pipeline(
            pipeline_id,
            jobs(&[root, left, right, join]),
            &[(root, left), (root, right), (left, join), (right, join)],
        )
        .await
        .unwrap();
    assert_eq!(job_state(&provider, root).await, JobState::Running);
    assert_eq!(job_state(&provider, left).await, JobState::Waiting);
    assert_eq!(job_state(&provider, join).await, JobState::Waiting);

    engine.job_completed(root).await.unwrap();
    assert_eq!(job_state(&provider, left).await, JobState::Running);
    assert_eq!(job_state(&provider, right).await, JobState::Running);
    engine.job_completed(left).await.unwrap();
    engine.job_completed(left).await.unwrap();
    assert_eq!(job_state(&provider, join).await, JobState::Waiting);
    let mut work = provider.begin().await.unwrap();
    assert!(work.edges().list_next(root).await.unwrap().is_empty());
    assert_eq!(work.edges().count_incoming(join).await.unwrap(), 1);
    work.rollback().await.unwrap();

    engine.job_completed(right).await.unwrap();
    assert_eq!(job_state(&provider, join).await, JobState::Running);
    engine.job_completed(join).await.unwrap();
    engine.job_failed(join).await.unwrap();
    engine.cancel_pipeline(pipeline_id).await.unwrap();
    assert_eq!(
        pipeline_state(&provider, pipeline_id).await,
        PipelineState::Completed
    );
    for job_id in [root, left, right, join] {
        assert_eq!(callback.count(job_id), 1);
    }
}

#[tokio::test]
async fn failed_start_can_be_retried_by_a_fresh_engine_without_blocking_other_roots() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let callback = RecordingCallback::new(&provider);
    let engine = Engine::new(provider.clone(), callback.clone());
    let pipeline_id = PipelineId::new();
    let [first, second] = std::array::from_fn(|_| JobId::new());
    *callback.fail.lock().unwrap() = Some(first);
    assert!(
        engine
            .start_pipeline(pipeline_id, jobs(&[first, second]), &[])
            .await
            .is_err()
    );
    assert_eq!(job_state(&provider, first).await, JobState::Starting);
    assert_eq!(job_state(&provider, second).await, JobState::Running);

    *callback.fail.lock().unwrap() = None;
    let engine = Engine::new(provider.clone(), callback.clone());
    engine
        .start_pipeline(pipeline_id, jobs(&[first, second]), &[])
        .await
        .unwrap();
    assert_eq!(callback.count(first), 2);
    assert_eq!(callback.count(second), 1);
    assert_eq!(job_state(&provider, first).await, JobState::Running);
}

#[tokio::test]
async fn duplicate_completion_retries_persisted_dispatch_even_after_edges_are_removed() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let callback = RecordingCallback::new(&provider);
    let engine = Engine::new(provider.clone(), callback.clone());
    let pipeline_id = PipelineId::new();
    let [root, child] = std::array::from_fn(|_| JobId::new());
    engine
        .start_pipeline(pipeline_id, jobs(&[root, child]), &[(root, child)])
        .await
        .unwrap();
    *callback.fail.lock().unwrap() = Some(child);
    assert!(engine.job_completed(root).await.is_err());
    assert_eq!(job_state(&provider, root).await, JobState::Completed);
    assert_eq!(job_state(&provider, child).await, JobState::Starting);

    *callback.fail.lock().unwrap() = None;
    let engine = Engine::new(provider.clone(), callback.clone());
    engine.job_completed(root).await.unwrap();
    assert_eq!(job_state(&provider, child).await, JobState::Running);
    assert_eq!(callback.count(root), 1);
    assert_eq!(callback.count(child), 2);
    engine.job_completed(child).await.unwrap();
    assert_eq!(
        pipeline_state(&provider, pipeline_id).await,
        PipelineState::Completed
    );
}

#[tokio::test]
async fn failure_and_cancellation_stop_successors_and_ignore_late_notifications() {
    for failed in [true, false] {
        let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
        let callback = RecordingCallback::new(&provider);
        let engine = Engine::new(provider.clone(), callback.clone());
        let pipeline_id = PipelineId::new();
        let [root, child] = std::array::from_fn(|_| JobId::new());
        engine
            .start_pipeline(pipeline_id, jobs(&[root, child]), &[(root, child)])
            .await
            .unwrap();
        if failed {
            engine.job_failed(root).await.unwrap();
        } else {
            engine.cancel_pipeline(pipeline_id).await.unwrap();
        }
        engine.cancel_pipeline(pipeline_id).await.unwrap();
        engine.job_completed(root).await.unwrap();
        engine.job_failed(root).await.unwrap();
        engine
            .start_pipeline(pipeline_id, jobs(&[root, child]), &[(root, child)])
            .await
            .unwrap();
        assert_eq!(
            pipeline_state(&provider, pipeline_id).await,
            PipelineState::Cancelled
        );
        assert_eq!(
            job_state(&provider, root).await,
            if failed {
                JobState::Failed
            } else {
                JobState::Running
            }
        );
        assert_eq!(job_state(&provider, child).await, JobState::Waiting);
        assert_eq!(callback.count(child), 0);
    }
}

#[tokio::test]
async fn waiting_jobs_cannot_complete_and_success_cannot_be_overwritten_by_failure() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let callback = RecordingCallback::new(&provider);
    let engine = Engine::new(provider.clone(), callback);
    let pipeline_id = PipelineId::new();
    let [root, child] = std::array::from_fn(|_| JobId::new());
    engine
        .start_pipeline(pipeline_id, jobs(&[root, child]), &[(root, child)])
        .await
        .unwrap();
    assert!(engine.job_completed(child).await.is_err());
    assert!(engine.job_failed(child).await.is_err());
    engine.job_completed(root).await.unwrap();
    engine.job_failed(root).await.unwrap();
    assert_eq!(job_state(&provider, root).await, JobState::Completed);
    assert_eq!(
        pipeline_state(&provider, pipeline_id).await,
        PipelineState::Running
    );
}

#[tokio::test]
async fn invalid_graphs_leave_no_pipeline_or_job_records() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let callback = RecordingCallback::new(&provider);
    let engine = Engine::new(provider.clone(), callback.clone());
    let [first, second, missing] = std::array::from_fn(|_| JobId::new());
    for (job_ids, edges) in [
        (vec![first, first], vec![]),
        (vec![first], vec![(first, missing)]),
        (vec![first], vec![(missing, first)]),
        (vec![first, second], vec![(first, second), (first, second)]),
        (vec![first], vec![(first, first)]),
        (
            vec![first, second, missing],
            vec![(first, second), (second, first)],
        ),
    ] {
        let pipeline_id = PipelineId::new();
        assert!(
            engine
                .start_pipeline(pipeline_id, jobs(&job_ids), &edges)
                .await
                .is_err()
        );
        let mut work = provider.begin().await.unwrap();
        assert!(matches!(
            work.pipelines().get(pipeline_id).await,
            Err(GetPipelineError::NotFound(_))
        ));
        assert!(work.jobs().get(first).await.is_err());
        work.rollback().await.unwrap();
    }
    assert!(callback.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn failed_registration_rolls_back_all_new_records() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let callback = RecordingCallback::new(&provider);
    let engine = Engine::new(provider.clone(), callback.clone());
    let existing = JobId::new();
    engine
        .start_pipeline(PipelineId::new(), jobs(&[existing]), &[])
        .await
        .unwrap();
    let pipeline_id = PipelineId::new();
    let created = JobId::new();
    assert!(
        engine
            .start_pipeline(pipeline_id, jobs(&[created, existing]), &[])
            .await
            .is_err()
    );
    let mut work = provider.begin().await.unwrap();
    assert!(matches!(
        work.pipelines().get(pipeline_id).await,
        Err(GetPipelineError::NotFound(_))
    ));
    assert!(work.jobs().get(created).await.is_err());
    work.rollback().await.unwrap();
    assert_eq!(callback.count(created), 0);
    assert!(engine.cancel_pipeline(PipelineId::new()).await.is_err());
    assert!(engine.job_completed(JobId::new()).await.is_err());
    assert!(engine.job_failed(JobId::new()).await.is_err());
}

#[tokio::test]
async fn competing_success_and_failure_notifications_accept_only_one_outcome() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let callback = RecordingCallback::new(&provider);
    let engine = Arc::new(Engine::new(provider.clone(), callback));
    let pipeline_id = PipelineId::new();
    let [root, child] = std::array::from_fn(|_| JobId::new());
    engine
        .start_pipeline(pipeline_id, jobs(&[root, child]), &[(root, child)])
        .await
        .unwrap();
    let completed_engine = engine.clone();
    let failed_engine = engine.clone();
    let completed = tokio::spawn(async move { completed_engine.job_completed(root).await });
    let failed = tokio::spawn(async move { failed_engine.job_failed(root).await });
    completed.await.unwrap().unwrap();
    failed.await.unwrap().unwrap();
    match job_state(&provider, root).await {
        JobState::Completed => {
            assert_eq!(
                pipeline_state(&provider, pipeline_id).await,
                PipelineState::Running
            );
            assert_eq!(job_state(&provider, child).await, JobState::Running);
        }
        JobState::Failed => {
            assert_eq!(
                pipeline_state(&provider, pipeline_id).await,
                PipelineState::Cancelled
            );
            assert_eq!(job_state(&provider, child).await, JobState::Waiting);
        }
        state => panic!("unexpected state: {state:?}"),
    }
}

#[derive(Serialize, Deserialize)]
struct InputV1 {
    command: String,
}

impl Upcast<String> for InputV1 {
    type Error = std::convert::Infallible;

    fn upcast(command: String) -> Result<Self, Self::Error> {
        Ok(Self { command })
    }
}

struct UpcastingCallback(Arc<Mutex<Vec<(JobId, String)>>>);

#[async_trait]
impl JobStartCallback for UpcastingCallback {
    type Input = Revision<InputV1, Revision0<String>>;
    type Error = io::Error;

    async fn start(&self, job_id: JobId, payload: Payload<Self::Input>) -> Result<(), Self::Error> {
        self.0
            .lock()
            .unwrap()
            .push((job_id, payload.into_value().command));
        Ok(())
    }
}

#[tokio::test]
async fn fresh_engine_upcasts_stored_inputs_for_retries_and_successors() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let callback = RecordingCallback::new(&provider);
    let engine = Engine::new(provider.clone(), callback.clone());
    let pipeline_id = PipelineId::new();
    let [root, child] = std::array::from_fn(|_| JobId::new());
    *callback.fail.lock().unwrap() = Some(root);
    assert!(
        engine
            .start_pipeline(pipeline_id, jobs(&[root, child]), &[(root, child)])
            .await
            .is_err()
    );
    let calls = Arc::new(Mutex::new(Vec::new()));
    let engine = Engine::new(provider.clone(), UpcastingCallback(calls.clone()));
    let latest = [root, child]
        .into_iter()
        .map(|job_id| {
            (
                job_id,
                Payload::new(InputV1 {
                    command: job_id.to_string(),
                })
                .unwrap(),
            )
        })
        .collect();
    engine
        .start_pipeline(pipeline_id, latest, &[(root, child)])
        .await
        .unwrap();
    engine.job_completed(root).await.unwrap();
    assert_eq!(
        *calls.lock().unwrap(),
        vec![(root, root.to_string()), (child, child.to_string())]
    );
    let mut work = provider.begin().await.unwrap();
    assert_eq!(work.jobs().get_payload(child).await.unwrap()["revision"], 0);
    work.rollback().await.unwrap();
}

#[tokio::test]
async fn empty_pipeline_completes_without_dispatch_and_existing_inputs_cannot_be_changed() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let callback = RecordingCallback::new(&provider);
    let engine = Engine::new(provider.clone(), callback.clone());
    let empty = PipelineId::new();
    engine.start_pipeline(empty, Vec::new(), &[]).await.unwrap();
    assert_eq!(
        pipeline_state(&provider, empty).await,
        PipelineState::Completed
    );
    assert!(callback.calls.lock().unwrap().is_empty());

    let pipeline_id = PipelineId::new();
    let job_id = JobId::new();
    engine
        .start_pipeline(pipeline_id, jobs(&[job_id]), &[])
        .await
        .unwrap();
    assert!(
        engine
            .start_pipeline(
                pipeline_id,
                vec![(job_id, Payload::new("changed".to_owned()).unwrap())],
                &[]
            )
            .await
            .is_err()
    );
    assert_eq!(callback.count(job_id), 1);
}

struct CompletingCallback(RecordingCallback);

#[async_trait]
impl JobStartCallback for CompletingCallback {
    type Input = Revision0<String>;
    type Error = io::Error;

    async fn start(&self, job_id: JobId, payload: Payload<Self::Input>) -> Result<(), Self::Error> {
        self.0.start(job_id, payload).await?;
        Engine::new(self.0.provider.clone(), self.0.clone())
            .job_completed(job_id)
            .await
            .map_err(io::Error::other)
    }
}

#[tokio::test]
async fn completion_inside_callback_is_not_overwritten_by_running() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let engine = Engine::new(
        provider.clone(),
        CompletingCallback(RecordingCallback::new(&provider)),
    );
    let pipeline_id = PipelineId::new();
    let job_id = JobId::new();
    engine
        .start_pipeline(pipeline_id, jobs(&[job_id]), &[])
        .await
        .unwrap();
    assert_eq!(job_state(&provider, job_id).await, JobState::Completed);
    assert_eq!(
        pipeline_state(&provider, pipeline_id).await,
        PipelineState::Completed
    );
}

#[tokio::test]
async fn concurrent_parent_completions_release_the_join() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let callback = RecordingCallback::new(&provider);
    let engine = Arc::new(Engine::new(provider.clone(), callback.clone()));
    let pipeline_id = PipelineId::new();
    let [first, second, join] = std::array::from_fn(|_| JobId::new());
    engine
        .start_pipeline(
            pipeline_id,
            jobs(&[first, second, join]),
            &[(first, join), (second, join)],
        )
        .await
        .unwrap();
    let first_engine = engine.clone();
    let second_engine = engine.clone();
    let first_completion = tokio::spawn(async move { first_engine.job_completed(first).await });
    let second_completion = tokio::spawn(async move { second_engine.job_completed(second).await });
    first_completion.await.unwrap().unwrap();
    second_completion.await.unwrap().unwrap();
    assert_eq!(job_state(&provider, join).await, JobState::Running);
    assert!(callback.count(join) >= 1);
}

struct BlockingCallback(Arc<AtomicBool>);

#[async_trait]
impl JobStartCallback for BlockingCallback {
    type Input = Revision0<String>;
    type Error = io::Error;

    async fn start(
        &self,
        _job_id: JobId,
        _payload: Payload<Self::Input>,
    ) -> Result<(), Self::Error> {
        self.0.store(true, Ordering::SeqCst);
        std::future::pending().await
    }
}

#[tokio::test]
async fn abandoned_start_is_recoverable_without_the_original_engine() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let entered = Arc::new(AtomicBool::new(false));
    let engine = Engine::new(provider.clone(), BlockingCallback(entered.clone()));
    let pipeline_id = PipelineId::new();
    let job_id = JobId::new();
    let task = tokio::spawn(async move {
        engine
            .start_pipeline(pipeline_id, jobs(&[job_id]), &[])
            .await
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        while !entered.load(Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(job_state(&provider, job_id).await, JobState::Starting);
    let engine = Engine::new(provider.clone(), RecordingCallback::new(&provider));
    engine
        .start_pipeline(pipeline_id, jobs(&[job_id]), &[])
        .await
        .unwrap();
    assert_eq!(job_state(&provider, job_id).await, JobState::Running);
}

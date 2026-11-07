use std::io;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use traojudge_core::application::judge_contract::INIT_SCRIPT_ENVIRONMENT_VARIABLE;
use traojudge_core::application::judge_contract::one_player::{
    OnePlayerJudgeContract, Realisation, SubmitError,
};
use traojudge_core::domain::model::judge_dag::{
    Artifact, ArtifactKind, ExecutionInput, ExecutionNode, JudgeDag, NodeName,
};
use traojudge_core::ports::judge_payload_builder::JudgePayloadBuilder;
use traojudge_workflow::payload::{Payload, PayloadError, Revision0};
use traojudge_workflow::storage::inmemory::InMemoryUnitOfWorkProvider;
use traojudge_workflow::storage::{PipelineRepository, UnitOfWork, UnitOfWorkProvider};
use traojudge_workflow::{Engine, JobId, JobStartCallback, PipelineState};

#[derive(Clone, Default)]
struct RecordingStart {
    jobs: Arc<Mutex<Vec<(JobId, String)>>>,
}

#[derive(Default)]
struct NamePayloadBuilder {
    prefix: String,
    calls: Vec<usize>,
}

impl JudgePayloadBuilder for NamePayloadBuilder {
    type Input = Revision0<String>;

    fn make_payload(
        &mut self,
        index: usize,
        execution: &ExecutionNode,
    ) -> Result<Payload<Self::Input>, PayloadError> {
        self.calls.push(index);
        Payload::new(format!("{}{}", self.prefix, execution.name))
    }
}

#[async_trait]
impl JobStartCallback for RecordingStart {
    type Input = Revision0<String>;
    type Error = io::Error;

    async fn start(&self, job_id: JobId, payload: Payload<Self::Input>) -> io::Result<()> {
        self.jobs
            .lock()
            .unwrap()
            .push((job_id, payload.into_value()));
        Ok(())
    }
}

fn name(value: &str) -> NodeName {
    NodeName::new(value).unwrap()
}

fn script_input() -> ExecutionInput {
    ExecutionInput {
        artifact: name("script"),
        environment_variable: Some(INIT_SCRIPT_ENVIRONMENT_VARIABLE.into()),
    }
}

fn dag() -> JudgeDag<Realisation> {
    JudgeDag {
        artifacts: vec![
            Artifact {
                name: name("script"),
                kind: ArtifactKind::Input {
                    realisation: Realisation::File {
                        path: "run.sh".into(),
                    },
                },
            },
            Artifact {
                name: name("first"),
                kind: ArtifactKind::Intermediate,
            },
            Artifact {
                name: name("second"),
                kind: ArtifactKind::Intermediate,
            },
        ],
        executions: vec![
            ExecutionNode {
                name: name("consume"),
                inputs: vec![
                    script_input(),
                    ExecutionInput {
                        artifact: name("first"),
                        environment_variable: Some("FIRST".into()),
                    },
                    ExecutionInput {
                        artifact: name("second"),
                        environment_variable: Some("SECOND".into()),
                    },
                ],
                outputs: vec![],
            },
            ExecutionNode {
                name: name("produce"),
                inputs: vec![script_input()],
                outputs: vec![name("first"), name("second")],
            },
        ],
    }
}

#[tokio::test]
async fn service_submits_dag_dependencies_to_the_workflow_engine() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let callback = RecordingStart::default();
    let engine = Engine::new(provider.clone(), callback.clone());
    let mut builder = NamePayloadBuilder {
        prefix: "custom/".into(),
        ..Default::default()
    };
    let payload_builder: &mut dyn JudgePayloadBuilder<Input = Revision0<String>> = &mut builder;
    let pipeline_id = OnePlayerJudgeContract
        .start_dag(&engine, &dag(), payload_builder)
        .await
        .unwrap();
    assert_eq!(builder.calls, vec![0, 1]);
    let root = {
        let jobs = callback.jobs.lock().unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].1, "custom/produce");
        jobs[0].0
    };
    engine.job_completed(root).await.unwrap();
    let child = {
        let jobs = callback.jobs.lock().unwrap();
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[1].1, "custom/consume");
        jobs[1].0
    };
    engine.job_completed(child).await.unwrap();
    let mut work = provider.begin().await.unwrap();
    assert_eq!(
        work.pipelines().get(pipeline_id).await.unwrap(),
        PipelineState::Completed
    );
    work.rollback().await.unwrap();
}

#[tokio::test]
async fn validation_precedes_payload_creation_and_workflow_dispatch() {
    for structural_error in [false, true] {
        let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
        let callback = RecordingStart::default();
        let engine = Engine::new(provider, callback.clone());
        let mut dag = dag();
        if structural_error {
            dag.executions[0].inputs[1].artifact = name("missing");
        } else {
            dag.executions[1].inputs.push(ExecutionInput {
                artifact: name("script"),
                environment_variable: Some("TRAOJUDGE_UNKNOWN".into()),
            });
            dag.validate().unwrap();
        }
        let mut payload_builder = NamePayloadBuilder::default();
        let result = OnePlayerJudgeContract
            .start_dag(&engine, &dag, &mut payload_builder)
            .await;
        assert!(matches!(result, Err(SubmitError::Validation(_))));
        assert!(payload_builder.calls.is_empty());
        assert!(callback.jobs.lock().unwrap().is_empty());
    }
}

struct FailingPayloadBuilder {
    calls: Vec<usize>,
}

impl JudgePayloadBuilder for FailingPayloadBuilder {
    type Input = Revision0<String>;

    fn make_payload(
        &mut self,
        index: usize,
        execution: &ExecutionNode,
    ) -> Result<Payload<Self::Input>, PayloadError> {
        self.calls.push(index);
        if index == 1 {
            return Err(PayloadError::UnsupportedRevision {
                revision: 1,
                latest_revision: 0,
            });
        }
        Payload::new(execution.name.to_string())
    }
}

#[tokio::test]
async fn payload_builder_failure_prevents_workflow_dispatch() {
    let provider = InMemoryUnitOfWorkProvider::new().await.unwrap();
    let callback = RecordingStart::default();
    let engine = Engine::new(provider, callback.clone());
    let mut payload_builder = FailingPayloadBuilder { calls: Vec::new() };
    let result = OnePlayerJudgeContract
        .start_dag(&engine, &dag(), &mut payload_builder)
        .await;
    assert!(matches!(
        result,
        Err(SubmitError::Payload(
            PayloadError::UnsupportedRevision { .. }
        ))
    ));
    assert_eq!(payload_builder.calls, vec![0, 1]);
    assert!(callback.jobs.lock().unwrap().is_empty());
}

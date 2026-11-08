use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;

use anyhow::{Context, Result, ensure};
use async_trait::async_trait;
use serde::Serialize;
use traojudge_core::application::judge_contract::SubmissionEnvironment;
use traojudge_core::application::judge_contract::one_player::{
    NodeOutput, OnePlayerJudgeContract, Realisation,
};
use traojudge_core::domain::model::judge_dag::{ArtifactKind, ExecutionNode, JudgeDag, NodeName};
use traojudge_core::ports::judge_payload_builder::JudgePayloadBuilder;
use traojudge_workflow::payload::{Payload, PayloadError, Revision0};
use traojudge_workflow::storage::inmemory::InMemoryUnitOfWorkProvider;
use traojudge_workflow::storage::{PipelineRepository, UnitOfWork, UnitOfWorkProvider};
use traojudge_workflow::{Engine, JobId, JobStartCallback, PipelineState};

use crate::definition::Definition;

#[derive(Debug, Serialize)]
pub struct RunReport {
    pub work_dir: PathBuf,
    pub nodes: Vec<NodeReport>,
}

impl RunReport {
    pub fn success(&self) -> bool {
        self.nodes
            .iter()
            .all(|node| node.status == NodeStatus::Succeeded)
    }
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    Succeeded,
    Failed,
    Skipped,
}

#[derive(Debug, Serialize)]
pub struct NodeReport {
    pub name: String,
    pub status: NodeStatus,
    exit_code: Option<i32>,
    result: Option<NodeOutput>,
    error: Option<String>,
}

pub async fn run(
    definition: &Definition,
    problem: &Path,
    submission: &Path,
    submission_environment: &SubmissionEnvironment,
    work_dir: &Path,
) -> Result<RunReport> {
    submission_environment.validate()?;
    let problem = problem
        .canonicalize()
        .context("failed to resolve problem path")?;
    let problem_dir = problem
        .parent()
        .context("problem path has no parent directory")?;
    let submission = submission
        .canonicalize()
        .context("failed to resolve submission path")?;
    ensure!(submission.is_file(), "submission must be a regular file");
    let dags = definition
        .dags
        .iter()
        .map(|dag| {
            ensure!(
                dag.environment == "host",
                "unsupported local environment {:?}; expected host",
                dag.environment
            );
            dag.to_domain()
        })
        .collect::<Result<Vec<_>>>()?;
    let mut sources = BTreeMap::new();
    for dag in &dags {
        for artifact in &dag.artifacts {
            if let ArtifactKind::Input { realisation } = &artifact.kind {
                let path = match realisation {
                    Realisation::File { path } => problem_dir.join(path),
                    Realisation::Submission {} => submission.clone(),
                };
                ensure!(
                    path.is_file(),
                    "input {} is not a regular file: {}",
                    artifact.name,
                    path.display()
                );
                sources.insert(artifact.name.clone(), path);
            }
        }
    }
    if let Some(parent) = work_dir
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(work_dir)
        .with_context(|| format!("work directory must be new: {}", work_dir.display()))?;
    let work_dir = work_dir.canonicalize()?;
    fs::create_dir(work_dir.join("artifacts"))?;
    fs::create_dir(work_dir.join("nodes"))?;
    let mut available = BTreeMap::new();
    for (name, source) in sources {
        let target = work_dir.join("artifacts").join(name.as_str());
        fs::copy(&source, &target).with_context(|| format!("failed to realise {name}"))?;
        available.insert(name, target);
    }
    let mut report = RunReport {
        work_dir,
        nodes: Vec::new(),
    };
    for dag in dags {
        run_dag(&dag, &mut available, submission_environment, &mut report).await?;
    }
    let report_file = File::create(report.work_dir.join("report.json"))?;
    serde_json::to_writer_pretty(report_file, &report)?;
    Ok(report)
}

struct LocalPayloadBuilder;

impl JudgePayloadBuilder for LocalPayloadBuilder {
    type Input = Revision0<usize>;

    fn make_payload(
        &mut self,
        index: usize,
        _execution: &ExecutionNode,
    ) -> Result<Payload<Self::Input>, PayloadError> {
        Payload::new(index)
    }
}

struct LocalJobStart {
    sender: mpsc::Sender<(JobId, usize)>,
}

#[async_trait]
impl JobStartCallback for LocalJobStart {
    type Input = Revision0<usize>;
    type Error = io::Error;

    async fn start(&self, job_id: JobId, payload: Payload<Self::Input>) -> io::Result<()> {
        self.sender
            .send((job_id, payload.into_value()))
            .map_err(io::Error::other)
    }
}

async fn run_dag(
    dag: &JudgeDag<Realisation>,
    available: &mut BTreeMap<NodeName, PathBuf>,
    submission_environment: &SubmissionEnvironment,
    report: &mut RunReport,
) -> Result<()> {
    let provider = InMemoryUnitOfWorkProvider::new().await?;
    let (sender, receiver) = mpsc::channel();
    let engine = Engine::new(provider.clone(), LocalJobStart { sender });
    let mut payload_builder = LocalPayloadBuilder;
    let pipeline_id = OnePlayerJudgeContract
        .start_dag(&engine, dag, &mut payload_builder)
        .await?;
    let mut executed = vec![false; dag.executions.len()];
    while let Ok((job_id, index)) = receiver.try_recv() {
        let execution = &dag.executions[index];
        let node_dir = report.work_dir.join("nodes").join(execution.name.as_str());
        let mut node = NodeReport {
            name: execution.name.to_string(),
            status: NodeStatus::Failed,
            exit_code: None,
            result: None,
            error: None,
        };
        let outcome = execute(
            execution,
            available,
            submission_environment,
            &node_dir,
            &mut node,
        )
        .and_then(|outputs| {
            outputs
                .into_iter()
                .map(|(name, path)| {
                    let target = report.work_dir.join("artifacts").join(name.as_str());
                    fs::copy(path, &target).with_context(|| format!("failed to collect {name}"))?;
                    Ok((name, target))
                })
                .collect::<Result<Vec<_>>>()
        });
        executed[index] = true;
        match outcome {
            Ok(outputs) => {
                available.extend(outputs);
                node.status = NodeStatus::Succeeded;
                report.nodes.push(node);
                engine.job_completed(job_id).await?;
            }
            Err(error) => {
                node.error = Some(format!("{error:#}"));
                report.nodes.push(node);
                engine.job_failed(job_id).await?;
                break;
            }
        }
    }
    let mut work = provider.begin().await?;
    let state = work.pipelines().get(pipeline_id).await?;
    work.rollback().await?;
    ensure!(
        matches!(state, PipelineState::Completed | PipelineState::Cancelled),
        "workflow stopped before the pipeline finished"
    );
    for (execution, executed) in dag.executions.iter().zip(executed) {
        if !executed {
            report.nodes.push(NodeReport {
                name: execution.name.to_string(),
                status: NodeStatus::Skipped,
                exit_code: None,
                result: None,
                error: Some("workflow pipeline was cancelled after a node failed".into()),
            });
        }
    }
    Ok(())
}

fn execute(
    execution: &ExecutionNode,
    available: &BTreeMap<NodeName, PathBuf>,
    submission_environment: &SubmissionEnvironment,
    node_dir: &Path,
    node: &mut NodeReport,
) -> Result<Vec<(NodeName, PathBuf)>> {
    fs::create_dir(node_dir)?;
    let input_dir = node_dir.join("inputs");
    let output_dir = node_dir.join("outputs");
    fs::create_dir(&input_dir)?;
    fs::create_dir(&output_dir)?;
    let mut command = Command::new("sh");
    command.env_clear();
    if let Some(path) = std::env::var_os("PATH") {
        command.env("PATH", path);
    }
    command.envs(submission_environment.environment_variables());
    for input in &execution.inputs {
        let target = input_dir.join(input.artifact.as_str());
        if !target.exists() {
            fs::copy(&available[&input.artifact], &target)?;
        }
        if let Some(variable) = &input.environment_variable {
            command.env(variable, &target);
        }
    }
    let script = execution
        .init_script()
        .context("execution has no init script")?;
    let stdout_path = node_dir.join("stdout");
    let stderr_path = node_dir.join("stderr");
    let status = command
        .arg(input_dir.join(script.artifact.as_str()))
        .current_dir(node_dir)
        .stdin(Stdio::null())
        .stdout(File::create(&stdout_path)?)
        .stderr(File::create(&stderr_path)?)
        .status()
        .context("failed to start init script with sh")?;
    node.exit_code = status.code();
    ensure!(status.success(), "init script exited with {status}");
    let result: NodeOutput = serde_json::from_reader(File::open(stdout_path)?)
        .context("init script stdout must be a JSON object")?;
    node.result = Some(result);
    execution
        .outputs
        .iter()
        .map(|name| {
            let path = output_dir.join(name.as_str());
            let metadata = fs::symlink_metadata(&path)
                .with_context(|| format!("missing declared output {name}"))?;
            ensure!(
                metadata.is_file(),
                "declared output {name} must be a regular file"
            );
            Ok((name.clone(), path))
        })
        .collect()
}

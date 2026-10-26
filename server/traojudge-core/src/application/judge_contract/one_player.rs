use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};
use traojudge_workflow::payload::PayloadError;
use traojudge_workflow::storage::UnitOfWorkProvider;
use traojudge_workflow::{Engine, JobId, JobStartCallback, PipelineId, StartPipelineError};

use super::{
    ContractError, SUBMISSION_LANGUAGE_ENVIRONMENT_VARIABLE,
    SUBMISSION_SOURCE_ENVIRONMENT_VARIABLE, SubmissionEnvironment,
};
use crate::domain::model::judge_dag::{
    ArtifactKind, DagError, JudgeDag, NodeName, ProblemJudgeDags,
};
use crate::ports::judge_payload_builder::JudgePayloadBuilder;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Realisation {
    File { path: String },
    Submission {},
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(transparent)]
pub struct NodeOutput(BTreeMap<String, serde_json::Value>);

impl NodeOutput {
    pub fn fields(&self) -> &BTreeMap<String, serde_json::Value> {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubmissionParameters {
    language: Option<String>,
}

impl SubmissionParameters {
    pub fn new(language: Option<String>) -> Result<Self, OnePlayerError> {
        if language
            .as_ref()
            .is_some_and(|language| language.trim().is_empty())
        {
            return Err(OnePlayerError::EmptyLanguage);
        }
        Ok(Self { language })
    }

    pub fn environment(&self) -> Result<SubmissionEnvironment, ContractError> {
        let variables = self
            .language
            .iter()
            .map(|language| {
                (
                    SUBMISSION_LANGUAGE_ENVIRONMENT_VARIABLE.to_owned(),
                    language.clone(),
                )
            })
            .collect();
        SubmissionEnvironment::new(variables)
    }
}

pub struct OnePlayerJudgeContract;

impl OnePlayerJudgeContract {
    pub fn validate_problem(
        &self,
        problem: &ProblemJudgeDags<Realisation>,
    ) -> Result<(), OnePlayerError> {
        problem.validate()?;
        for dag in &problem.dags {
            self.validate_contract(dag)?;
        }
        Ok(())
    }

    pub fn validate_dag(&self, dag: &JudgeDag<Realisation>) -> Result<(), OnePlayerError> {
        dag.validate()?;
        self.validate_contract(dag)
    }

    fn validate_contract(&self, dag: &JudgeDag<Realisation>) -> Result<(), OnePlayerError> {
        for artifact in &dag.artifacts {
            if let ArtifactKind::Input {
                realisation: Realisation::File { path },
            } = &artifact.kind
            {
                if path.trim().is_empty() {
                    return Err(OnePlayerError::EmptyFilePath(artifact.name.clone()));
                }
            }
        }
        let artifacts: BTreeMap<_, _> = dag
            .artifacts
            .iter()
            .map(|artifact| (&artifact.name, &artifact.kind))
            .collect();
        for execution in &dag.executions {
            super::validate_execution(execution)?;
            for input in &execution.inputs {
                if input.environment_variable.as_deref()
                    == Some(SUBMISSION_SOURCE_ENVIRONMENT_VARIABLE)
                    && !matches!(
                        artifacts[&input.artifact],
                        ArtifactKind::Input {
                            realisation: Realisation::Submission {}
                        }
                    )
                {
                    return Err(OnePlayerError::InvalidSubmissionSource {
                        execution: execution.name.clone(),
                        artifact: input.artifact.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    pub async fn start_dag<Provider, Callback, Builder>(
        &self,
        engine: &Engine<Provider, Callback>,
        dag: &JudgeDag<Realisation>,
        payload_builder: &mut Builder,
    ) -> Result<PipelineId, SubmitError<Provider::Error, Callback::Error>>
    where
        Provider: UnitOfWorkProvider,
        Callback: JobStartCallback,
        Builder: JudgePayloadBuilder<Input = Callback::Input> + ?Sized,
    {
        self.validate_dag(dag)?;
        let pipeline_id = PipelineId::new();
        let job_ids: Vec<_> = dag.executions.iter().map(|_| JobId::new()).collect();
        let producers: BTreeMap<_, _> = dag
            .executions
            .iter()
            .zip(&job_ids)
            .flat_map(|(execution, &job_id)| {
                execution.outputs.iter().map(move |name| (name, job_id))
            })
            .collect();
        let mut edges = Vec::new();
        let mut seen = HashSet::new();
        for (execution, &job_id) in dag.executions.iter().zip(&job_ids) {
            for input in &execution.inputs {
                if let Some(&producer) = producers.get(&input.artifact) {
                    let edge = (producer, job_id);
                    if seen.insert(edge) {
                        edges.push(edge);
                    }
                }
            }
        }
        let jobs = dag
            .executions
            .iter()
            .zip(job_ids)
            .enumerate()
            .map(|(index, (execution, job_id))| {
                Ok((job_id, payload_builder.make_payload(index, execution)?))
            })
            .collect::<Result<Vec<_>, PayloadError>>()?;
        engine.start_pipeline(pipeline_id, jobs, &edges).await?;
        Ok(pipeline_id)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SubmitError<
    StorageError: std::error::Error + 'static,
    CallbackError: std::error::Error + 'static,
> {
    #[error(transparent)]
    Validation(#[from] OnePlayerError),
    #[error(transparent)]
    Payload(#[from] PayloadError),
    #[error(transparent)]
    Workflow(#[from] StartPipelineError<StorageError, CallbackError>),
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum OnePlayerError {
    #[error(transparent)]
    Dag(#[from] DagError),
    #[error(transparent)]
    Contract(#[from] ContractError),
    #[error("submission language must not be empty")]
    EmptyLanguage,
    #[error("input artifact {0} has an empty file path")]
    EmptyFilePath(NodeName),
    #[error("execution {execution} binds {artifact} to {variable}, but it is not a submission input", variable = SUBMISSION_SOURCE_ENVIRONMENT_VARIABLE)]
    InvalidSubmissionSource {
        execution: NodeName,
        artifact: NodeName,
    },
}

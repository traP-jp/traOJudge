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

pub trait OnePlayerRealisation: Clone + Send + Sync + 'static {
    fn validate(&self, artifact: &NodeName) -> Result<(), OnePlayerError>;
    fn is_submission(&self) -> bool;
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Realisation {
    File { path: String },
    Submission {},
}

impl OnePlayerRealisation for Realisation {
    fn validate(&self, artifact: &NodeName) -> Result<(), OnePlayerError> {
        match self {
            Self::File { path } => {
                if path.trim().is_empty() {
                    return Err(OnePlayerError::EmptyFilePath(artifact.clone()));
                }
                Ok(())
            }
            Self::Submission {} => Ok(()),
        }
    }

    fn is_submission(&self) -> bool {
        matches!(self, Self::Submission {})
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UploadedObjectRealisation {
    Object {
        bucket: String,
        key: String,
        hash: String,
    },
    Submission {},
}

impl UploadedObjectRealisation {
    pub fn object(
        bucket: impl Into<String>,
        key: impl Into<String>,
        hash: impl Into<String>,
    ) -> Self {
        Self::Object {
            bucket: bucket.into(),
            key: key.into(),
            hash: hash.into(),
        }
    }
}

fn is_valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

impl OnePlayerRealisation for UploadedObjectRealisation {
    fn validate(&self, artifact: &NodeName) -> Result<(), OnePlayerError> {
        match self {
            Self::Object { bucket, key, hash } => {
                if bucket.trim().is_empty() {
                    return Err(OnePlayerError::EmptyBucket(artifact.clone()));
                }
                if key.trim().is_empty() {
                    return Err(OnePlayerError::EmptyObjectKey(artifact.clone()));
                }
                if !is_valid_sha256(hash) {
                    return Err(OnePlayerError::InvalidObjectHash(artifact.clone()));
                }
                Ok(())
            }
            Self::Submission {} => Ok(()),
        }
    }

    fn is_submission(&self) -> bool {
        matches!(self, Self::Submission {})
    }
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
    pub fn validate_problem<R: OnePlayerRealisation>(
        &self,
        problem: &ProblemJudgeDags<R>,
    ) -> Result<(), OnePlayerError> {
        problem.validate()?;
        for dag in &problem.dags {
            self.validate_contract(dag)?;
        }
        Ok(())
    }

    pub fn validate_dag<R: OnePlayerRealisation>(
        &self,
        dag: &JudgeDag<R>,
    ) -> Result<(), OnePlayerError> {
        dag.validate()?;
        self.validate_contract(dag)
    }

    fn validate_contract<R: OnePlayerRealisation>(
        &self,
        dag: &JudgeDag<R>,
    ) -> Result<(), OnePlayerError> {
        for artifact in &dag.artifacts {
            if let ArtifactKind::Input { realisation } = &artifact.kind {
                realisation.validate(&artifact.name)?;
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
                {
                    let is_submission = match artifacts.get(&input.artifact) {
                        Some(ArtifactKind::Input { realisation }) => realisation.is_submission(),
                        _ => false,
                    };
                    if !is_submission {
                        return Err(OnePlayerError::InvalidSubmissionSource {
                            execution: execution.name.clone(),
                            artifact: input.artifact.clone(),
                        });
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn start_dag<R, Provider, Callback, Builder>(
        &self,
        engine: &Engine<Provider, Callback>,
        dag: &JudgeDag<R>,
        payload_builder: &mut Builder,
    ) -> Result<PipelineId, SubmitError<Provider::Error, Callback::Error>>
    where
        R: OnePlayerRealisation,
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
    #[error("input artifact {0} has an empty bucket")]
    EmptyBucket(NodeName),
    #[error("input artifact {0} has an empty object key")]
    EmptyObjectKey(NodeName),
    #[error("input artifact {0} has an invalid object hash")]
    InvalidObjectHash(NodeName),
    #[error("execution {execution} binds {artifact} to {variable}, but it is not a submission input", variable = SUBMISSION_SOURCE_ENVIRONMENT_VARIABLE)]
    InvalidSubmissionSource {
        execution: NodeName,
        artifact: NodeName,
    },
}

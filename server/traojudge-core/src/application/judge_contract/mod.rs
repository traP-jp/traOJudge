use std::collections::BTreeMap;

pub use crate::domain::model::judge_dag::INIT_SCRIPT_ENVIRONMENT_VARIABLE;
use crate::domain::model::judge_dag::{DagError, ExecutionNode, validate_environment_variable};

pub mod one_player;

pub const SERVICE_ENVIRONMENT_PREFIX: &str = "TRAOJUDGE_";
pub const SUBMISSION_SOURCE_ENVIRONMENT_VARIABLE: &str = "TRAOJUDGE_SUBMISSION_SOURCE";
pub const SUBMISSION_LANGUAGE_ENVIRONMENT_VARIABLE: &str = "TRAOJUDGE_SUBMISSION_LANGUAGE";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReservedEnvironmentVariable {
    InitScript,
    SubmissionSource,
    SubmissionLanguage,
}

impl ReservedEnvironmentVariable {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            INIT_SCRIPT_ENVIRONMENT_VARIABLE => Some(Self::InitScript),
            SUBMISSION_SOURCE_ENVIRONMENT_VARIABLE => Some(Self::SubmissionSource),
            SUBMISSION_LANGUAGE_ENVIRONMENT_VARIABLE => Some(Self::SubmissionLanguage),
            _ => None,
        }
    }

    pub fn is_artifact_input(self) -> bool {
        matches!(self, Self::InitScript | Self::SubmissionSource)
    }
}

pub fn validate_execution(execution: &ExecutionNode) -> Result<(), ContractError> {
    for input in &execution.inputs {
        if let Some(variable) = &input.environment_variable {
            validate_environment_variable(variable)?;
            if variable.starts_with(SERVICE_ENVIRONMENT_PREFIX) {
                let reserved =
                    ReservedEnvironmentVariable::from_name(variable).ok_or_else(|| {
                        ContractError::UnknownReservedEnvironmentVariable(variable.clone())
                    })?;
                if !reserved.is_artifact_input() {
                    return Err(ContractError::ReservedEnvironmentVariable(variable.clone()));
                }
            }
        }
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubmissionEnvironment {
    pub environment_variables: BTreeMap<String, String>,
}

impl SubmissionEnvironment {
    pub fn new(environment_variables: BTreeMap<String, String>) -> Result<Self, ContractError> {
        let environment = Self {
            environment_variables,
        };
        environment.validate()?;
        Ok(environment)
    }

    pub fn validate(&self) -> Result<(), ContractError> {
        for variable in self.environment_variables.keys() {
            validate_environment_variable(variable)?;
            if !variable.starts_with(SERVICE_ENVIRONMENT_PREFIX) {
                return Err(ContractError::NonServiceEnvironmentVariable(
                    variable.clone(),
                ));
            }
            let reserved = ReservedEnvironmentVariable::from_name(variable).ok_or_else(|| {
                ContractError::UnknownReservedEnvironmentVariable(variable.clone())
            })?;
            if reserved.is_artifact_input() {
                return Err(ContractError::ReservedEnvironmentVariable(variable.clone()));
            }
        }
        Ok(())
    }

    pub fn environment_variables(&self) -> &BTreeMap<String, String> {
        &self.environment_variables
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ContractError {
    #[error(transparent)]
    Dag(#[from] DagError),
    #[error("environment variable {0} is reserved by the service")]
    ReservedEnvironmentVariable(String),
    #[error("unknown reserved environment variable: {0}")]
    UnknownReservedEnvironmentVariable(String),
    #[error("submission parameter {0} must use {prefix}", prefix = SERVICE_ENVIRONMENT_PREFIX)]
    NonServiceEnvironmentVariable(String),
}

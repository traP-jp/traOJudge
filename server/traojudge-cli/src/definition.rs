use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use traojudge_core::application::judge_contract::one_player::{
    OnePlayerJudgeContract, Realisation,
};
use traojudge_core::domain::model::judge_dag::{
    Artifact, ArtifactKind, ExecutionInput, ExecutionNode, JudgeDag, NodeName, ProblemJudgeDags,
};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub dags: Vec<DagDefinition>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DagDefinition {
    pub environment: String,
    artifacts: Vec<ArtifactDefinition>,
    executions: Vec<ExecutionDefinition>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ArtifactDefinition {
    Input { name: String, realise: Realisation },
    Intermediate { name: String },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ExecutionDefinition {
    name: String,
    inputs: Vec<InputDefinition>,
    outputs: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct InputDefinition {
    artifact: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    environment_variable: Option<String>,
}

pub fn load(path: &Path) -> Result<Definition> {
    let source = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    parse(&source).with_context(|| format!("invalid DAG definition: {}", path.display()))
}

fn parse(source: &str) -> Result<Definition> {
    let definition: Definition = serde_json::from_str(source).context("invalid JSON definition")?;
    definition.validate()?;
    Ok(definition)
}

impl Definition {
    fn validate(&self) -> Result<()> {
        ensure!(
            !self.dags.is_empty(),
            "definition must contain at least one DAG"
        );
        let dags = self
            .dags
            .iter()
            .map(DagDefinition::to_domain)
            .collect::<Result<Vec<_>>>()?;
        let problem = ProblemJudgeDags::new(dags)?;
        OnePlayerJudgeContract.validate_problem(&problem)?;
        Ok(())
    }
}

impl DagDefinition {
    pub fn to_domain(&self) -> Result<JudgeDag<Realisation>> {
        ensure!(
            !self.environment.trim().is_empty(),
            "environment must not be empty"
        );
        let artifacts = self
            .artifacts
            .iter()
            .map(|artifact| {
                let (name, kind) = match artifact {
                    ArtifactDefinition::Input { name, realise } => (
                        name,
                        ArtifactKind::Input {
                            realisation: realise.clone(),
                        },
                    ),
                    ArtifactDefinition::Intermediate { name } => (name, ArtifactKind::Intermediate),
                };
                Ok(Artifact {
                    name: NodeName::new(name.clone())?,
                    kind,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let executions = self
            .executions
            .iter()
            .map(|execution| {
                let inputs = execution
                    .inputs
                    .iter()
                    .map(|input| {
                        Ok(ExecutionInput {
                            artifact: NodeName::new(input.artifact.clone())?,
                            environment_variable: input.environment_variable.clone(),
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                let outputs = execution
                    .outputs
                    .iter()
                    .map(|name| NodeName::new(name.clone()))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(ExecutionNode {
                    name: NodeName::new(execution.name.clone())?,
                    inputs,
                    outputs,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(JudgeDag {
            artifacts,
            executions,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    const JSON_EXAMPLE: &str = include_str!("../examples/judge.json");
    fn assert_rejected(value: Value, expected: &str) {
        let json = serde_json::to_string(&value).unwrap();
        let error = parse(&json).unwrap_err();
        assert!(format!("{error:#}").contains(expected), "{error:#}");
    }

    #[test]
    fn rejects_unknown_fields() {
        let mut value: Value = serde_json::from_str(JSON_EXAMPLE).unwrap();
        value["dags"][0]["executions"][0]["outptus"] = json!([]);
        assert_rejected(value, "unknown field");
    }

    #[test]
    fn rejects_missing_init_script() {
        let mut value: Value = serde_json::from_str(JSON_EXAMPLE).unwrap();
        value["dags"][0]["executions"][0]["inputs"] = json!([{ "artifact": "source" }]);
        assert_rejected(value, "has no TRAOJUDGE_INIT_SCRIPT input");
    }

    #[test]
    fn rejects_cycles() {
        let mut value: Value = serde_json::from_str(JSON_EXAMPLE).unwrap();
        value["dags"][0]["executions"][0]["inputs"]
            .as_array_mut()
            .unwrap()
            .push(json!({ "artifact": "case_0_output" }));
        assert_rejected(value, "directed cycle");
    }

    #[test]
    fn rejects_duplicate_names_across_dags() {
        let mut value: Value = serde_json::from_str(JSON_EXAMPLE).unwrap();
        let duplicate = value["dags"][0].clone();
        value["dags"].as_array_mut().unwrap().push(duplicate);
        assert_rejected(value, "duplicate node name");
    }
}

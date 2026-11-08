//! Versioned, cloud-independent wire contract. Artifact bytes never travel in SQS.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use traojudge_core::{
    application::judge_contract::one_player::{OnePlayerJudgeContract, Realisation},
    domain::model::judge_dag::{
        Artifact, ArtifactKind, ExecutionInput, ExecutionNode, JudgeDag, NodeName,
    },
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ObjectRef {
    pub bucket: String,
    pub key: String,
    pub version_id: String,
    pub sha256: String,
    pub byte_length: u64,
    #[serde(default)]
    pub executable: bool,
}
impl ObjectRef {
    pub fn validate(&self, bucket: &str) -> Result<()> {
        ensure!(
            self.bucket == bucket
                && (self.key.starts_with("inputs/") || self.key.starts_with("runs/")),
            "object outside judge storage"
        );
        ensure!(
            !self.version_id.is_empty() && self.version_id != "null",
            "versioned object required"
        );
        ensure!(
            self.sha256.len() == 64
                && self
                    .sha256
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
            "invalid SHA-256"
        );
        ensure!(
            self.byte_length <= 512 * 1024 * 1024,
            "input exceeds 512 MiB"
        );
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactDefinition {
    pub name: String,
    #[serde(default)]
    pub reference: Option<ObjectRef>,
    #[serde(default)]
    pub submission: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputBinding {
    pub artifact: String,
    #[serde(default)]
    pub environment_variable: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub wall_seconds: u64,
    pub memory_mb: u64,
    pub cpus: f64,
    pub pids: u32,
    pub output_bytes: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            wall_seconds: 30,
            memory_mb: 256,
            cpus: 1.,
            pids: 128,
            output_bytes: 32 * 1024 * 1024,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Peer {
    pub name: String,
    pub script: String,
    pub inputs: Vec<String>,
    #[serde(default)]
    pub limits: Limits,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Execution {
    pub name: String,
    pub inputs: Vec<InputBinding>,
    pub outputs: Vec<String>,
    #[serde(default)]
    pub limits: Limits,
    #[serde(default)]
    pub peers: Vec<Peer>,
    #[serde(default)]
    pub main_inputs: Option<Vec<String>>,
    #[serde(default)]
    pub case_id: Option<String>,
    // A leaf can translate a supervised timeout/nonzero exit into a verdict.
    #[serde(default)]
    pub judge_errors: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub schema_version: u32,
    pub image: String,
    pub artifacts: Vec<ArtifactDefinition>,
    pub executions: Vec<Execution>,
}
impl Definition {
    pub fn domain(&self) -> Result<JudgeDag<Realisation>> {
        Ok(JudgeDag::new(
            self.artifacts
                .iter()
                .map(|a| {
                    Ok(Artifact {
                        name: NodeName::new(&a.name)?,
                        kind: match &a.reference {
                            Some(r) => ArtifactKind::Input {
                                realisation: if a.submission {
                                    Realisation::Submission {}
                                } else {
                                    Realisation::File {
                                        path: r.key.clone(),
                                    }
                                },
                            },
                            None => {
                                ensure!(!a.submission, "submission needs a reference");
                                ArtifactKind::Intermediate
                            }
                        },
                    })
                })
                .collect::<Result<_>>()?,
            self.executions
                .iter()
                .map(|e| {
                    Ok(ExecutionNode {
                        name: NodeName::new(&e.name)?,
                        inputs: e
                            .inputs
                            .iter()
                            .map(|i| {
                                Ok(ExecutionInput {
                                    artifact: NodeName::new(&i.artifact)?,
                                    environment_variable: i.environment_variable.clone(),
                                })
                            })
                            .collect::<Result<_>>()?,
                        outputs: e
                            .outputs
                            .iter()
                            .map(|n| NodeName::new(n).map_err(Into::into))
                            .collect::<Result<_>>()?,
                    })
                })
                .collect::<Result<_>>()?,
        )?)
    }
    pub fn validate(&self, bucket: &str, image: &str) -> Result<()> {
        ensure!(self.schema_version == 1, "unsupported definition schema");
        ensure!(
            self.image == image && (image.contains("@sha256:") || image.starts_with("sha256:")),
            "runtime image must match the pinned stack image"
        );
        ensure!(
            self.executions.len() <= 256 && self.artifacts.len() <= 2048,
            "definition too large"
        );
        OnePlayerJudgeContract.validate_dag(&self.domain()?)?;
        for a in &self.artifacts {
            if let Some(r) = &a.reference {
                r.validate(bucket)?;
            }
        }
        let mut cases = BTreeSet::new();
        for e in &self.executions {
            validate_limits(&e.limits)?;
            ensure!(
                !e.judge_errors || e.outputs.is_empty(),
                "judge_errors requires a leaf without outputs"
            );
            if let Some(c) = &e.case_id {
                NodeName::new(c)?;
                ensure!(cases.insert(c), "duplicate case ID");
            }
            let inputs: BTreeSet<_> = e.inputs.iter().map(|i| i.artifact.as_str()).collect();
            if let Some(main) = &e.main_inputs {
                for i in main {
                    ensure!(inputs.contains(i.as_str()), "unknown main input");
                }
                let script = e
                    .inputs
                    .iter()
                    .find(|i| i.environment_variable.as_deref() == Some("TRAOJUDGE_INIT_SCRIPT"))
                    .unwrap();
                ensure!(
                    main.contains(&script.artifact),
                    "main inputs must include init script"
                );
            }
            ensure!(e.peers.len() <= 8, "too many peers");
            let mut peers = BTreeSet::new();
            for p in &e.peers {
                NodeName::new(&p.name)?;
                ensure!(p.name != "main" && peers.insert(&p.name), "duplicate peer");
                ensure!(p.inputs.contains(&p.script), "peer script must be an input");
                for i in &p.inputs {
                    ensure!(inputs.contains(i.as_str()), "unknown peer input");
                }
                validate_limits(&p.limits)?;
            }
        }
        Ok(())
    }
}
fn validate_limits(l: &Limits) -> Result<()> {
    ensure!(
        (1..=300).contains(&l.wall_seconds)
            && (32..=1024).contains(&l.memory_mb)
            && l.cpus.is_finite()
            && l.cpus > 0.
            && l.cpus <= 2.
            && (16..=256).contains(&l.pids)
            && (1..=64 * 1024 * 1024).contains(&l.output_bytes),
        "limits outside worker capacity"
    );
    Ok(())
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Template {
    pub run_id: String,
    pub image: String,
    pub execution: Execution,
    pub inputs: BTreeMap<String, Option<ObjectRef>>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub schema_version: u32,
    pub run_id: String,
    pub job_id: String,
    pub image: String,
    pub execution: Execution,
    pub inputs: BTreeMap<String, ObjectRef>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NodeResult {
    pub schema_version: u32,
    pub run_id: String,
    pub job_id: String,
    pub node: String,
    pub outcome: String,
    pub result: Option<serde_json::Value>,
    pub outputs: BTreeMap<String, ObjectRef>,
    pub logs: BTreeMap<String, ObjectRef>,
    pub processes: BTreeMap<String, serde_json::Value>,
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn definition() -> Definition {
        Definition {
            schema_version: 1,
            image: format!("example@sha256:{}", "a".repeat(64)),
            artifacts: vec![ArtifactDefinition {
                name: "init".into(),
                reference: Some(ObjectRef {
                    bucket: "judge".into(),
                    key: "inputs/init".into(),
                    version_id: "v1".into(),
                    sha256: "0".repeat(64),
                    byte_length: 2,
                    executable: false,
                }),
                submission: false,
            }],
            executions: vec![Execution {
                name: "run".into(),
                inputs: vec![InputBinding {
                    artifact: "init".into(),
                    environment_variable: Some("TRAOJUDGE_INIT_SCRIPT".into()),
                }],
                outputs: vec![],
                limits: Limits::default(),
                peers: vec![],
                main_inputs: None,
                case_id: Some("case".into()),
                judge_errors: false,
            }],
        }
    }
    #[test]
    fn contract_requires_pinned_storage_and_runtime() {
        let mut d = definition();
        let image = d.image.clone();
        d.validate("judge", &image).unwrap();
        d.artifacts[0].reference.as_mut().unwrap().version_id = "null".into();
        assert!(d.validate("judge", &image).is_err());
        let mut d = definition();
        d.schema_version = 2;
        assert!(d.validate("judge", &image).is_err());
        let mut d = definition();
        d.image = "example:latest".into();
        assert!(d.validate("judge", &d.image).is_err());
    }
    #[test]
    fn session_rejects_undeclared_inputs_and_oversized_limits() {
        let mut d = definition();
        let image = d.image.clone();
        d.executions[0].peers.push(Peer {
            name: "solution".into(),
            script: "hidden".into(),
            inputs: vec!["hidden".into()],
            limits: Limits::default(),
        });
        assert!(d.validate("judge", &image).is_err());
        let mut d = definition();
        d.executions[0].limits.wall_seconds = 301;
        assert!(d.validate("judge", &image).is_err());
    }
}

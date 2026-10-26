use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

pub const INIT_SCRIPT_ENVIRONMENT_VARIABLE: &str = "TRAOJUDGE_INIT_SCRIPT";

/// アーティファクトと実行ノードに使う名前。
///
/// [`Self::new`]と[`Self::validate`]は、名前がASCIIの英数字・`_`・`-`だけからなる
/// 1〜64文字の文字列であること（`^[A-Za-z0-9_-]{1,64}$`）を保証する。
/// 問題全体での一意性は[`ProblemJudgeDags::validate`]で検証する。
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeName(String);

impl NodeName {
    pub fn new(value: impl Into<String>) -> Result<Self, DagError> {
        let name = Self(value.into());
        name.validate()?;
        Ok(name)
    }

    pub fn validate(&self) -> Result<(), DagError> {
        let value = &self.0;
        if value.is_empty()
            || value.len() > 64
            || !value.bytes().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, b'_' | b'-')
            })
        {
            return Err(DagError::InvalidName(value.clone()));
        }
        Ok(())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NodeName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// ファイルを表すノード。`RealisationMethod`は実体化方法を記録する値の型。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Artifact<RealisationMethod> {
    pub name: NodeName,
    pub kind: ArtifactKind<RealisationMethod>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArtifactKind<RealisationMethod> {
    Input { realisation: RealisationMethod },
    Intermediate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionInput {
    pub artifact: NodeName,
    pub environment_variable: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionNode {
    pub name: NodeName,
    pub inputs: Vec<ExecutionInput>,
    pub outputs: Vec<NodeName>,
}

impl ExecutionNode {
    pub fn init_script(&self) -> Option<&ExecutionInput> {
        self.inputs.iter().find(|input| {
            input.environment_variable.as_deref() == Some(INIT_SCRIPT_ENVIRONMENT_VARIABLE)
        })
    }
}

/// アーティファクトと実行ノードからなる二部DAG。
///
/// [`Self::validate`]が成功した時点で、次の不変条件を満たす。
///
/// - 全ノードの名前が[`NodeName`]の文字種・長さの制約を満たし、
///   アーティファクトと実行ノードを通じてDAG内で一意である。
/// - 実行ノードのすべての入出力参照が、このDAG内に存在するアーティファクトを指す。
/// - 入力ファイルの入次数は0、中間生成物ファイルの入次数はちょうど1である。
/// - 入出力参照が表す有向グラフに閉路がない。
/// - 入力に割り当てる環境変数名が`^[A-Za-z_][A-Za-z0-9_]*$`を満たし、
///   同じ実行ノード内で重複しない。
/// - 各実行ノードに、`TRAOJUDGE_INIT_SCRIPT`を割り当てた入力がちょうど1つ存在する。
///
/// [`Self::new`]は構築時に検証する。公開フィールドを直接構築・変更した場合は、
/// 利用前に[`Self::validate`]を呼ぶ必要がある。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JudgeDag<RealisationMethod> {
    pub artifacts: Vec<Artifact<RealisationMethod>>,
    pub executions: Vec<ExecutionNode>,
}

impl<RealisationMethod> JudgeDag<RealisationMethod> {
    pub fn new(
        artifacts: Vec<Artifact<RealisationMethod>>,
        executions: Vec<ExecutionNode>,
    ) -> Result<Self, DagError> {
        let dag = Self {
            artifacts,
            executions,
        };
        dag.validate()?;
        Ok(dag)
    }

    pub fn artifacts(&self) -> &[Artifact<RealisationMethod>] {
        &self.artifacts
    }

    pub fn executions(&self) -> &[ExecutionNode] {
        &self.executions
    }

    pub fn node_names(&self) -> impl Iterator<Item = &NodeName> {
        self.artifacts
            .iter()
            .map(|artifact| &artifact.name)
            .chain(self.executions.iter().map(|execution| &execution.name))
    }

    pub fn validate(&self) -> Result<(), DagError> {
        let mut names = BTreeSet::new();
        for name in self.node_names() {
            name.validate()?;
            if !names.insert(name) {
                return Err(DagError::DuplicateName(name.clone()));
            }
        }

        let artifact_indices: BTreeMap<_, _> = self
            .artifacts
            .iter()
            .enumerate()
            .map(|(index, artifact)| (&artifact.name, index))
            .collect();
        let node_count = self.artifacts.len() + self.executions.len();
        let mut successors = vec![Vec::new(); node_count];
        let mut incoming_edges = vec![0; node_count];

        for (execution_index, execution) in self.executions.iter().enumerate() {
            let execution_index = self.artifacts.len() + execution_index;
            let mut variables = BTreeSet::new();
            let mut input_artifacts = BTreeSet::new();
            for input in &execution.inputs {
                let artifact_index = *artifact_indices.get(&input.artifact).ok_or_else(|| {
                    DagError::UnknownArtifact {
                        execution: execution.name.clone(),
                        artifact: input.artifact.clone(),
                    }
                })?;
                if let Some(variable) = &input.environment_variable {
                    validate_environment_variable(variable)?;
                    if !variables.insert(variable.as_str()) {
                        return Err(DagError::DuplicateEnvironmentVariable {
                            execution: execution.name.clone(),
                            variable: variable.clone(),
                        });
                    }
                }
                if input_artifacts.insert(artifact_index) {
                    successors[artifact_index].push(execution_index);
                    incoming_edges[execution_index] += 1;
                }
            }
            if execution.init_script().is_none() {
                return Err(DagError::MissingInitScript(execution.name.clone()));
            }
            for output in &execution.outputs {
                let artifact_index =
                    *artifact_indices
                        .get(output)
                        .ok_or_else(|| DagError::UnknownArtifact {
                            execution: execution.name.clone(),
                            artifact: output.clone(),
                        })?;
                if matches!(
                    self.artifacts[artifact_index].kind,
                    ArtifactKind::Input { .. }
                ) {
                    return Err(DagError::InputHasProducer(output.clone()));
                }
                successors[execution_index].push(artifact_index);
                incoming_edges[artifact_index] += 1;
            }
        }

        for (index, artifact) in self.artifacts.iter().enumerate() {
            if matches!(artifact.kind, ArtifactKind::Intermediate) && incoming_edges[index] != 1 {
                return Err(DagError::InvalidProducerCount {
                    artifact: artifact.name.clone(),
                    count: incoming_edges[index],
                });
            }
        }

        let mut ready: VecDeque<_> = incoming_edges
            .iter()
            .enumerate()
            .filter_map(|(index, count)| (*count == 0).then_some(index))
            .collect();
        let mut visited = 0;
        while let Some(index) = ready.pop_front() {
            visited += 1;
            for &successor in &successors[index] {
                incoming_edges[successor] -= 1;
                if incoming_edges[successor] == 0 {
                    ready.push_back(successor);
                }
            }
        }
        if visited != node_count {
            return Err(DagError::Cycle);
        }
        Ok(())
    }
}

/// 同じ問題に属するジャッジDAGの集合。
///
/// [`Self::validate`]が成功した時点で、全DAGが[`JudgeDag::validate`]の不変条件を満たし、
/// 全DAGのアーティファクト名・実行ノード名が問題全体で一意である。
///
/// [`Self::new`]は構築時に検証する。公開フィールドを直接構築・変更した場合は、
/// 利用前に[`Self::validate`]を呼ぶ必要がある。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProblemJudgeDags<RealisationMethod> {
    pub dags: Vec<JudgeDag<RealisationMethod>>,
}

impl<RealisationMethod> ProblemJudgeDags<RealisationMethod> {
    pub fn new(dags: Vec<JudgeDag<RealisationMethod>>) -> Result<Self, DagError> {
        let problem = Self { dags };
        problem.validate()?;
        Ok(problem)
    }

    pub fn validate(&self) -> Result<(), DagError> {
        let mut names = BTreeSet::new();
        for dag in &self.dags {
            dag.validate()?;
            for name in dag.node_names() {
                if !names.insert(name) {
                    return Err(DagError::DuplicateName(name.clone()));
                }
            }
        }
        Ok(())
    }

    pub fn dags(&self) -> &[JudgeDag<RealisationMethod>] {
        &self.dags
    }
}

pub fn validate_environment_variable(variable: &str) -> Result<(), DagError> {
    let mut characters = variable.bytes();
    let valid_start = characters
        .next()
        .is_some_and(|character| character.is_ascii_alphabetic() || character == b'_');
    if !valid_start
        || !characters.all(|character| character.is_ascii_alphanumeric() || character == b'_')
    {
        return Err(DagError::InvalidEnvironmentVariable(variable.to_owned()));
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DagError {
    #[error("invalid node name: {0:?}")]
    InvalidName(String),
    #[error("duplicate node name: {0}")]
    DuplicateName(NodeName),
    #[error("execution {execution} references unknown artifact {artifact}")]
    UnknownArtifact {
        execution: NodeName,
        artifact: NodeName,
    },
    #[error("input artifact {0} has a producer")]
    InputHasProducer(NodeName),
    #[error("intermediate artifact {artifact} has {count} producers, expected 1")]
    InvalidProducerCount { artifact: NodeName, count: usize },
    #[error("execution {0} has no {variable} input", variable = INIT_SCRIPT_ENVIRONMENT_VARIABLE)]
    MissingInitScript(NodeName),
    #[error("invalid environment variable: {0:?}")]
    InvalidEnvironmentVariable(String),
    #[error("execution {execution} binds environment variable {variable} more than once")]
    DuplicateEnvironmentVariable {
        execution: NodeName,
        variable: String,
    },
    #[error("judge DAG contains a directed cycle")]
    Cycle,
}

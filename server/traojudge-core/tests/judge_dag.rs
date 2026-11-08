use traojudge_core::domain::model::judge_dag::{
    Artifact, ArtifactKind, DagError, ExecutionInput, ExecutionNode,
    INIT_SCRIPT_ENVIRONMENT_VARIABLE, JudgeDag, NodeName,
};

fn name(value: &str) -> NodeName {
    NodeName::new(value).unwrap()
}

fn dag() -> JudgeDag<()> {
    JudgeDag {
        artifacts: vec![Artifact {
            name: name("input"),
            kind: ArtifactKind::Input { realisation: () },
        }],
        executions: vec![ExecutionNode {
            name: name("run"),
            inputs: vec![
                ExecutionInput {
                    artifact: name("input"),
                    environment_variable: Some("TRAOJUDGE_FUTURE".into()),
                },
                ExecutionInput {
                    artifact: name("input"),
                    environment_variable: Some(INIT_SCRIPT_ENVIRONMENT_VARIABLE.into()),
                },
            ],
            outputs: vec![],
        }],
    }
}

#[test]
fn accepts_structurally_valid_dags_without_judge_contract_requirements() {
    let dag = dag();
    dag.validate().unwrap();
    assert_eq!(
        dag.executions[0].init_script().unwrap().artifact,
        name("input")
    );
    JudgeDag::new(dag.artifacts, dag.executions).unwrap();
}

#[test]
fn rejects_missing_init_script_as_a_dag_constraint() {
    let result = JudgeDag::<()>::new(
        vec![],
        vec![ExecutionNode {
            name: name("run"),
            inputs: vec![],
            outputs: vec![],
        }],
    );
    assert!(matches!(result, Err(DagError::MissingInitScript(_))));
}

#[test]
fn rejects_duplicate_init_script_bindings() {
    let mut dag = dag();
    let duplicate = dag.executions[0].inputs[1].clone();
    dag.executions[0].inputs.push(duplicate);
    assert!(
        matches!(dag.validate(), Err(DagError::DuplicateEnvironmentVariable { variable, .. })
        if variable == INIT_SCRIPT_ENVIRONMENT_VARIABLE)
    );
}

#[test]
fn still_checks_environment_variable_syntax_and_duplicate_bindings() {
    let mut dag = dag();
    dag.executions[0].inputs[0].environment_variable = Some("1INVALID".into());
    assert!(matches!(
        dag.validate(),
        Err(DagError::InvalidEnvironmentVariable(_))
    ));
    dag.executions[0].inputs[0].environment_variable = Some("VARIABLE".into());
    let duplicate = dag.executions[0].inputs[0].clone();
    dag.executions[0].inputs.push(duplicate);
    assert!(matches!(
        dag.validate(),
        Err(DagError::DuplicateEnvironmentVariable { .. })
    ));
}

#[test]
fn still_rejects_invalid_artifact_references_without_contract_validation() {
    let mut dag = dag();
    dag.executions[0].inputs[0].artifact = name("missing");
    assert!(matches!(
        dag.validate(),
        Err(DagError::UnknownArtifact { .. })
    ));
}

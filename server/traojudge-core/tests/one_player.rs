use std::collections::BTreeMap;

use serde_json::json;
use traojudge_core::application::judge_contract::one_player::{
    NodeOutput, OnePlayerError, OnePlayerJudgeContract, Realisation, SubmissionParameters,
};
use traojudge_core::application::judge_contract::{
    ContractError, INIT_SCRIPT_ENVIRONMENT_VARIABLE, SUBMISSION_LANGUAGE_ENVIRONMENT_VARIABLE,
    SUBMISSION_SOURCE_ENVIRONMENT_VARIABLE, SubmissionEnvironment,
};
use traojudge_core::domain::model::judge_dag::{
    Artifact, ArtifactKind, DagError, ExecutionInput, ExecutionNode, JudgeDag, NodeName,
    ProblemJudgeDags,
};

fn name(value: &str) -> NodeName {
    NodeName::new(value).unwrap()
}

fn problem() -> ProblemJudgeDags<Realisation> {
    ProblemJudgeDags::new(vec![JudgeDag {
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
                name: name("source"),
                kind: ArtifactKind::Input {
                    realisation: Realisation::Submission {},
                },
            },
        ],
        executions: vec![ExecutionNode {
            name: name("run"),
            inputs: vec![
                ExecutionInput {
                    artifact: name("script"),
                    environment_variable: Some(INIT_SCRIPT_ENVIRONMENT_VARIABLE.into()),
                },
                ExecutionInput {
                    artifact: name("source"),
                    environment_variable: Some(SUBMISSION_SOURCE_ENVIRONMENT_VARIABLE.into()),
                },
            ],
            outputs: vec![],
        }],
    }])
    .unwrap()
}

#[test]
fn accepts_one_player_source_binding_and_rejects_a_non_submission_artifact() {
    let mut problem = problem();
    OnePlayerJudgeContract.validate_problem(&problem).unwrap();
    problem.dags[0].executions[0].inputs[1].artifact = name("script");
    assert!(matches!(
        OnePlayerJudgeContract.validate_problem(&problem),
        Err(OnePlayerError::InvalidSubmissionSource { .. })
    ));
}

#[test]
fn validates_file_specifications_in_core() {
    let mut problem = problem();
    problem.dags[0].artifacts[0].kind = ArtifactKind::Input {
        realisation: Realisation::File { path: " ".into() },
    };
    assert_eq!(
        OnePlayerJudgeContract.validate_problem(&problem),
        Err(OnePlayerError::EmptyFilePath(name("script")))
    );
}

#[test]
fn rejects_unknown_reserved_names_and_submission_parameters_as_artifact_bindings() {
    for variable in [
        "TRAOJUDGE_UNKNOWN",
        SUBMISSION_LANGUAGE_ENVIRONMENT_VARIABLE,
    ] {
        let mut problem = problem();
        problem.dags[0].executions[0].inputs[1].environment_variable = Some(variable.into());
        problem.validate().unwrap();
        assert!(OnePlayerJudgeContract.validate_problem(&problem).is_err());
    }
}

#[test]
fn missing_init_script_is_reported_as_a_dag_error() {
    let mut problem = problem();
    problem.dags[0].executions[0].inputs.remove(0);
    assert!(matches!(
        problem.validate(),
        Err(DagError::MissingInitScript(_))
    ));
    assert!(matches!(
        OnePlayerJudgeContract.validate_problem(&problem),
        Err(OnePlayerError::Dag(DagError::MissingInitScript(_)))
    ));
}

#[test]
fn submission_parameters_use_the_core_reserved_name_without_changing_the_language_id() {
    let environment = SubmissionParameters::new(Some("cpp-23-gcc".into()))
        .unwrap()
        .environment()
        .unwrap();
    assert_eq!(
        environment.environment_variables()[SUBMISSION_LANGUAGE_ENVIRONMENT_VARIABLE],
        "cpp-23-gcc"
    );
    assert!(
        SubmissionParameters::new(None)
            .unwrap()
            .environment()
            .unwrap()
            .environment_variables()
            .is_empty()
    );
    assert_eq!(
        SubmissionParameters::new(Some(" ".into())),
        Err(OnePlayerError::EmptyLanguage)
    );
}

#[test]
fn submission_environment_rejects_input_bindings_and_undefined_reserved_parameters() {
    for variable in [
        INIT_SCRIPT_ENVIRONMENT_VARIABLE,
        SUBMISSION_SOURCE_ENVIRONMENT_VARIABLE,
        "TRAOJUDGE_UNKNOWN",
        "LANGUAGE",
    ] {
        assert!(
            SubmissionEnvironment::new(BTreeMap::from([(variable.into(), "value".into())]))
                .is_err()
        );
    }
    assert!(matches!(
        SubmissionEnvironment::new(BTreeMap::from([(
            "TRAOJUDGE_UNKNOWN".into(),
            "value".into()
        )])),
        Err(ContractError::UnknownReservedEnvironmentVariable(_))
    ));
}

#[test]
fn node_output_is_an_object_and_preserves_writer_defined_result_fields() {
    for value in [
        json!({}),
        json!({"verdict": "accepted", "score": 42, "details": ["case_1"]}),
    ] {
        let output: NodeOutput = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(&output).unwrap(), value);
        assert_eq!(output.fields().len(), value.as_object().unwrap().len());
    }
    for value in [
        json!(null),
        json!([]),
        json!(42),
        json!("accepted"),
        json!(true),
    ] {
        assert!(serde_json::from_value::<NodeOutput>(value).is_err());
    }
}

#[test]
fn realisation_uses_the_shared_submission_keyword_and_rejects_unknown_fields() {
    assert_eq!(
        serde_json::from_value::<Realisation>(json!({"kind": "submission"})).unwrap(),
        Realisation::Submission {}
    );
    assert!(
        serde_json::from_value::<Realisation>(json!({"kind": "submission", "player": 2})).is_err()
    );
    assert!(serde_json::from_value::<Realisation>(json!({"kind": "other"})).is_err());
}

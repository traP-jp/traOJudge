#![cfg(unix)]

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

use serde_json::{Value, json};

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("traojudge-local-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("solution.sh"), "printf '42\\n'\n").unwrap();
        fs::write(
            root.join("produce.sh"),
            "set -eu\nsh \"$SOURCE\" > outputs/answer\nprintf '{}\\n'\n",
        )
        .unwrap();
        fs::write(
            root.join("consume.sh"),
            "set -eu\ncat \"$ANSWER\" > outputs/checked\nprintf '{\"verdict\":\"accepted\"}\\n'\n",
        )
        .unwrap();
        fs::write(
            root.join("problem.json"),
            serde_json::to_vec_pretty(&Self::definition()).unwrap(),
        )
        .unwrap();
        Self { root }
    }

    fn definition() -> Value {
        json!({"dags": [{
            "environment": "host",
            "artifacts": [
                {"kind": "input", "name": "produce_script", "realise": {"kind": "file", "path": "produce.sh"}},
                {"kind": "input", "name": "consume_script", "realise": {"kind": "file", "path": "consume.sh"}},
                {"kind": "input", "name": "source", "realise": {"kind": "submission"}},
                {"kind": "intermediate", "name": "answer"},
                {"kind": "intermediate", "name": "checked"}
            ],
            "executions": [
                {"name": "consume", "inputs": [
                    {"artifact": "consume_script", "environment_variable": "TRAOJUDGE_INIT_SCRIPT"},
                    {"artifact": "answer", "environment_variable": "ANSWER"}
                ], "outputs": ["checked"]},
                {"name": "produce", "inputs": [
                    {"artifact": "produce_script", "environment_variable": "TRAOJUDGE_INIT_SCRIPT"},
                    {"artifact": "source", "environment_variable": "SOURCE"}
                ], "outputs": ["answer"]}
            ]
        }]})
    }

    fn run(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_traojudge-cli"))
            .current_dir(std::env::temp_dir())
            .arg("run")
            .arg(self.root.join("problem.json"))
            .arg("--submission")
            .arg(self.root.join("solution.sh"))
            .arg("--work-dir")
            .arg(self.root.join("work"))
            .output()
            .unwrap()
    }

    fn report(&self) -> Value {
        serde_json::from_slice(&fs::read(self.root.join("work/report.json")).unwrap()).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn runs_in_dependency_order_with_problem_relative_files_and_submission() {
    let fixture = Fixture::new();
    let output = fixture.run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = fixture.report();
    assert_eq!(report["nodes"][0]["name"], "produce");
    assert_eq!(report["nodes"][1]["name"], "consume");
    assert_eq!(report["nodes"][1]["result"]["verdict"], "accepted");
    assert_eq!(
        fs::read_to_string(fixture.root.join("work/artifacts/checked")).unwrap(),
        "42\n"
    );
    assert_eq!(
        fs::read_to_string(fixture.root.join("solution.sh")).unwrap(),
        "printf '42\\n'\n"
    );
}

#[test]
fn failed_node_skips_descendants_and_still_runs_an_independent_dag() {
    let fixture = Fixture::new();
    fs::write(
        fixture.root.join("produce.sh"),
        "echo failure >&2\nexit 7\n",
    )
    .unwrap();
    fs::write(
        fixture.root.join("independent.sh"),
        "printf '{\"ok\":true}\\n'\n",
    )
    .unwrap();
    let mut problem = Fixture::definition();
    problem["dags"].as_array_mut().unwrap().push(json!({
        "environment": "host",
        "artifacts": [{"kind": "input", "name": "independent_script", "realise": {"kind": "file", "path": "independent.sh"}}],
        "executions": [{"name": "independent", "inputs": [{"artifact": "independent_script", "environment_variable": "TRAOJUDGE_INIT_SCRIPT"}], "outputs": []}]
    }));
    fs::write(
        fixture.root.join("problem.json"),
        serde_json::to_vec(&problem).unwrap(),
    )
    .unwrap();
    assert!(!fixture.run().status.success());
    let report = fixture.report();
    assert_eq!(report["nodes"][0]["status"], "failed");
    assert_eq!(report["nodes"][0]["exit_code"], 7);
    assert_eq!(report["nodes"][1]["status"], "skipped");
    assert_eq!(report["nodes"][2]["status"], "succeeded");
    assert!(
        fs::read_to_string(fixture.root.join("work/nodes/produce/stderr"))
            .unwrap()
            .contains("failure")
    );
    assert!(!fixture.root.join("work/nodes/consume").exists());
}

#[test]
fn missing_output_and_invalid_stdout_fail_the_node() {
    for (script, message) in [
        ("printf '{}\\n'\n", "missing declared output answer"),
        ("echo diagnostic\n", "stdout must be a JSON object"),
    ] {
        let fixture = Fixture::new();
        fs::write(fixture.root.join("produce.sh"), script).unwrap();
        assert!(!fixture.run().status.success());
        let report = fixture.report();
        assert_eq!(report["nodes"][0]["status"], "failed");
        assert!(
            report["nodes"][0]["error"]
                .as_str()
                .unwrap()
                .contains(message)
        );
        assert_eq!(report["nodes"][1]["status"], "skipped");
    }
}

#[test]
fn rejects_existing_work_directory_and_missing_inputs_before_execution() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.root.join("work")).unwrap();
    fs::write(fixture.root.join("work/keep"), "keep").unwrap();
    let output = fixture.run();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("work directory must be new"));
    assert_eq!(
        fs::read_to_string(fixture.root.join("work/keep")).unwrap(),
        "keep"
    );
    fs::remove_dir_all(fixture.root.join("work")).unwrap();
    fs::remove_file(fixture.root.join("consume.sh")).unwrap();
    assert!(!fixture.run().status.success());
    assert!(!fixture.root.join("work").exists());
}

#[test]
fn rejects_unknown_environment_instead_of_ignoring_it() {
    let fixture = Fixture::new();
    let mut problem = Fixture::definition();
    problem["dags"][0]["environment"] = json!("unknown-environment");
    fs::write(
        fixture.root.join("problem.json"),
        serde_json::to_vec(&problem).unwrap(),
    )
    .unwrap();
    let output = fixture.run();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unsupported local environment"));
    assert!(!fixture.root.join("work").exists());
}

#[test]
fn copies_inputs_per_node_and_keeps_unbound_inputs_accessible() {
    let fixture = Fixture::new();
    fs::write(
        fixture.root.join("produce.sh"),
        "set -eu\necho changed > \"$SOURCE\"\ncat inputs/source > outputs/answer\nprintf '{}\\n'\n",
    )
    .unwrap();
    let output = fixture.run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(fixture.root.join("work/artifacts/source")).unwrap(),
        "printf '42\\n'\n"
    );
    assert_eq!(
        fs::read_to_string(fixture.root.join("work/artifacts/checked")).unwrap(),
        "changed\n"
    );
}

#[test]
fn supports_an_intermediate_artifact_as_init_script() {
    let fixture = Fixture::new();
    fs::write(
        fixture.root.join("produce.sh"),
        "set -eu\nprintf \"printf '{}\\\\n'\\n\" > outputs/answer\nprintf '{}\\n'\n",
    )
    .unwrap();
    let mut problem = Fixture::definition();
    problem["dags"][0]["executions"][0]["inputs"] = json!([
        {"artifact": "answer", "environment_variable": "TRAOJUDGE_INIT_SCRIPT"}
    ]);
    problem["dags"][0]["executions"][0]["outputs"] = json!([]);
    problem["dags"][0]["artifacts"]
        .as_array_mut()
        .unwrap()
        .retain(|artifact| artifact["name"] != "checked");
    fs::write(
        fixture.root.join("problem.json"),
        serde_json::to_vec(&problem).unwrap(),
    )
    .unwrap();
    let output = fixture.run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fixture.report()["nodes"][1]["status"], "succeeded");
}

#[test]
fn workflow_waits_for_all_parents_and_deduplicates_file_dependencies() {
    let fixture = Fixture::new();
    fs::write(fixture.root.join("produce.sh"),
        "set -eu\nsh \"$SOURCE\" > outputs/answer\ncp outputs/answer outputs/answer_copy\nprintf '{}\\n'\n").unwrap();
    fs::write(fixture.root.join("left.sh"),
        "set -eu\ncmp \"$ANSWER\" \"$COPY\"\ncmp \"$ANSWER\" \"$AGAIN\"\ncat \"$ANSWER\" > outputs/left\nprintf '{}\\n'\n").unwrap();
    fs::write(
        fixture.root.join("right.sh"),
        "set -eu\ncat \"$ANSWER\" > outputs/right\nprintf '{}\\n'\n",
    )
    .unwrap();
    fs::write(
        fixture.root.join("consume.sh"),
        "set -eu\ncat \"$LEFT\" \"$RIGHT\" > outputs/checked\nprintf '{}\\n'\n",
    )
    .unwrap();
    let mut problem = Fixture::definition();
    problem["dags"][0]["artifacts"].as_array_mut().unwrap().extend([
        json!({"kind": "intermediate", "name": "answer_copy"}),
        json!({"kind": "intermediate", "name": "left"}),
        json!({"kind": "intermediate", "name": "right"}),
        json!({"kind": "input", "name": "left_script", "realise": {"kind": "file", "path": "left.sh"}}),
        json!({"kind": "input", "name": "right_script", "realise": {"kind": "file", "path": "right.sh"}}),
    ]);
    problem["dags"][0]["executions"][0]["inputs"] = json!([
        {"artifact": "consume_script", "environment_variable": "TRAOJUDGE_INIT_SCRIPT"},
        {"artifact": "left", "environment_variable": "LEFT"},
        {"artifact": "right", "environment_variable": "RIGHT"}
    ]);
    problem["dags"][0]["executions"][1]["outputs"] = json!(["answer", "answer_copy"]);
    problem["dags"][0]["executions"]
        .as_array_mut()
        .unwrap()
        .extend([
            json!({"name": "left", "inputs": [
            {"artifact": "left_script", "environment_variable": "TRAOJUDGE_INIT_SCRIPT"},
            {"artifact": "answer", "environment_variable": "ANSWER"},
            {"artifact": "answer_copy", "environment_variable": "COPY"},
            {"artifact": "answer", "environment_variable": "AGAIN"}
        ], "outputs": ["left"]}),
            json!({"name": "right", "inputs": [
            {"artifact": "right_script", "environment_variable": "TRAOJUDGE_INIT_SCRIPT"},
            {"artifact": "answer", "environment_variable": "ANSWER"}
        ], "outputs": ["right"]}),
        ]);
    problem["dags"][0]["executions"][2]["name"] = json!("make_left");
    problem["dags"][0]["executions"][3]["name"] = json!("make_right");
    fs::write(
        fixture.root.join("problem.json"),
        serde_json::to_vec(&problem).unwrap(),
    )
    .unwrap();
    let output = fixture.run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = fixture.report();
    let nodes = report["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 4);
    assert_eq!(nodes[0]["name"], "produce");
    assert_eq!(nodes[3]["name"], "consume");
    assert!(nodes.iter().all(|node| node["status"] == "succeeded"));
    assert_eq!(
        fs::read_to_string(fixture.root.join("work/artifacts/checked")).unwrap(),
        "42\n42\n"
    );
}

#[test]
fn workflow_cancels_unexecuted_nodes_in_the_same_dag_after_failure() {
    let fixture = Fixture::new();
    fs::write(fixture.root.join("produce.sh"), "exit 7\n").unwrap();
    let mut problem = Fixture::definition();
    problem["dags"][0]["executions"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "name": "independent", "inputs": [
                {"artifact": "consume_script", "environment_variable": "TRAOJUDGE_INIT_SCRIPT"}
            ], "outputs": []
        }));
    fs::write(fixture.root.join("consume.sh"), "printf '{}\\n'\n").unwrap();
    fs::write(
        fixture.root.join("problem.json"),
        serde_json::to_vec(&problem).unwrap(),
    )
    .unwrap();
    assert!(!fixture.run().status.success());
    let report = fixture.report();
    let nodes = report["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 3);
    let failure = nodes
        .iter()
        .position(|node| node["status"] == "failed")
        .unwrap();
    assert_eq!(nodes[failure]["name"], "produce");
    assert!(
        nodes[failure + 1..]
            .iter()
            .all(|node| node["status"] == "skipped")
    );
    assert!(!fixture.root.join("work/nodes/consume").exists());
}

#[test]
fn workflow_completes_a_dag_without_execution_nodes() {
    let fixture = Fixture::new();
    fs::write(
        fixture.root.join("problem.json"),
        serde_json::to_vec(&json!({
            "dags": [{"environment": "host", "artifacts": [], "executions": []}]
        }))
        .unwrap(),
    )
    .unwrap();
    let output = fixture.run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fixture.report()["nodes"], json!([]));
}

#[test]
fn passes_core_one_player_source_and_language_to_the_script() {
    let fixture = Fixture::new();
    let mut problem = Fixture::definition();
    problem["dags"][0]["executions"][1]["inputs"][1]["environment_variable"] =
        json!("TRAOJUDGE_SUBMISSION_SOURCE");
    fs::write(
        fixture.root.join("problem.json"),
        serde_json::to_vec(&problem).unwrap(),
    )
    .unwrap();
    fs::write(fixture.root.join("produce.sh"),
        "set -eu\n[ \"$TRAOJUDGE_SUBMISSION_LANGUAGE\" = 'shell-test' ]\nsh \"$TRAOJUDGE_SUBMISSION_SOURCE\" > outputs/answer\nprintf '{}\\n'\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_traojudge-cli"))
        .arg("run")
        .arg(fixture.root.join("problem.json"))
        .arg("--submission")
        .arg(fixture.root.join("solution.sh"))
        .arg("--language")
        .arg("shell-test")
        .arg("--work-dir")
        .arg(fixture.root.join("work"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(fixture.root.join("work/artifacts/checked")).unwrap(),
        "42\n"
    );
}

#[test]
fn rejects_invalid_one_player_source_bindings_before_creating_the_work_directory() {
    let fixture = Fixture::new();
    let mut problem = Fixture::definition();
    problem["dags"][0]["executions"][1]["inputs"][0]["environment_variable"] =
        json!("TRAOJUDGE_SUBMISSION_SOURCE");
    problem["dags"][0]["executions"][1]["inputs"][1]["environment_variable"] =
        json!("TRAOJUDGE_INIT_SCRIPT");
    fs::write(
        fixture.root.join("problem.json"),
        serde_json::to_vec(&problem).unwrap(),
    )
    .unwrap();
    let output = fixture.run();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("not a submission input"));
    assert!(!fixture.root.join("work").exists());
}

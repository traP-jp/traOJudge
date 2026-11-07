use std::path::Path;
use std::process::Command;

#[test]
fn validates_and_emits_json() {
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("../examples/judge.json")).unwrap();
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/judge.json");
    let output = Command::new(env!("CARGO_BIN_EXE_traojudge-cli"))
        .arg("validate")
        .arg(path)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let actual: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn invalid_input_exits_unsuccessfully_without_json_output() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let output = Command::new(env!("CARGO_BIN_EXE_traojudge-cli"))
        .arg("validate")
        .arg(path)
        .arg("--json")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid JSON definition"));
}

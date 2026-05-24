use std::process::Command;

fn fixture(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

fn run(args: &[&str]) -> (String, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_wiretrail"))
        .args(args)
        .output()
        .expect("binary runs");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        out.status.code().unwrap_or(-1),
    )
}

#[test]
fn diagnose_json_envelope() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "diagnose", "--json"]);
    assert!(stdout.contains("\"command\": \"diagnose\""));
    assert!(stdout.contains("\"findings\""));
}

#[test]
fn validate_json_envelope() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "validate", "--json"]);
    assert!(stdout.contains("\"command\": \"validate\""));
    assert!(stdout.contains("\"sanitized\""));
}

#[test]
fn startup_text_header() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "startup"]);
    assert!(stdout.contains("== wiretrail startup =="));
    assert!(stdout.contains("max concurrency"));
}

#[test]
fn cascade_text_header() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "cascade"]);
    assert!(stdout.contains("== wiretrail cascade =="));
}

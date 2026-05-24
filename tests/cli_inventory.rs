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
fn hosts_text_has_header() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "hosts"]);
    assert!(stdout.contains("== wiretrail hosts =="));
    assert!(stdout.contains("api.someapi123.io"));
}

#[test]
fn subsystems_json_envelope() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "subsystems", "--json"]);
    assert!(stdout.contains("\"command\": \"subsystems\""));
    assert!(stdout.contains("\"subsystems\""));
}

#[test]
fn endpoints_json_envelope() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "endpoints", "--json"]);
    assert!(stdout.contains("\"command\": \"endpoints\""));
    assert!(stdout.contains("\"endpoints\""));
}

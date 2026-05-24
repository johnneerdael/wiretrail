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
fn jwt_json_envelope() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "jwt", "--json"]);
    assert!(stdout.contains("\"command\": \"jwt\""));
    assert!(stdout.contains("\"tokens\""));
}

#[test]
fn auth_json_envelope() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "auth", "--json"]);
    assert!(stdout.contains("\"command\": \"auth\""));
    assert!(stdout.contains("\"failures\""));
    assert!(stdout.contains("\"refreshes\""));
}

#[test]
fn handoff_text_header() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "handoff"]);
    assert!(stdout.contains("== wiretrail handoff =="));
}

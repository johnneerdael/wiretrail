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
fn diff_json_envelope() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "diff", "--json"]);
    assert!(stdout.contains("\"command\": \"diff\""));
    assert!(stdout.contains("\"groups\""));
}

#[test]
fn checks_json_envelope() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "checks", "--json"]);
    assert!(stdout.contains("\"command\": \"checks\""));
    assert!(stdout.contains("\"findings\""));
}

#[test]
fn checks_text_header() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "checks"]);
    assert!(stdout.contains("== wiretrail checks =="));
}

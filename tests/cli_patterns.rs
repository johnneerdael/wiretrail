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
fn storms_text_header() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "storms"]);
    assert!(stdout.contains("== wiretrail storms =="));
}

#[test]
fn pagination_json_envelope() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "pagination", "--json"]);
    assert!(stdout.contains("\"command\": \"pagination\""));
    assert!(stdout.contains("\"pages\""));
    assert!(stdout.contains("\"nplus1\""));
}

#[test]
fn rate_limit_json_envelope() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "rate-limit", "--json"]);
    assert!(stdout.contains("\"command\": \"rate-limit\""));
    assert!(stdout.contains("\"groups\""));
}

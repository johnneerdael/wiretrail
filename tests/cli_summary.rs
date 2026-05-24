use std::process::Command;

fn fixture(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

fn run(args: &[&str]) -> (String, String, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_wiretrail"))
        .args(args)
        .output()
        .expect("binary runs");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.code().unwrap_or(-1),
    )
}

#[test]
fn default_summary_prints_header() {
    let (stdout, _stderr, _code) = run(&[&fixture("someapi123.har")]);
    assert!(stdout.contains("== wiretrail summary =="));
    assert!(stdout.contains("top hosts"));
}

#[test]
fn json_envelope_is_stable() {
    let (stdout, _stderr, _code) = run(&[&fixture("someapi123.har"), "summary", "--json"]);
    assert!(stdout.contains("\"tool\": \"wiretrail\""));
    assert!(stdout.contains("\"schema_version\": 1"));
    assert!(stdout.contains("\"command\": \"summary\""));
    assert!(stdout.contains("\"next_commands\""));
}

#[test]
fn invalid_file_exits_2() {
    let (_stdout, stderr, code) = run(&["/nonexistent/path.har"]);
    assert_eq!(code, 2);
    assert!(stderr.contains("wiretrail:"));
}

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
fn rules_json_envelope() {
    let (stdout, _) = run(&[
        &fixture("someapi123.har"),
        "rules",
        "--pack",
        "auth",
        "--json",
    ]);
    assert!(stdout.contains("\"command\": \"rules\""));
    assert!(stdout.contains("\"findings\""));
}

#[test]
fn compare_json_envelope() {
    let (stdout, _) = run(&[
        &fixture("someapi123.har"),
        "compare",
        &fixture("someapi13.har"),
        "--json",
    ]);
    assert!(stdout.contains("\"command\": \"compare\""));
    assert!(stdout.contains("\"max_severity\""));
}

#[test]
fn compare_against_self_is_clean() {
    let (stdout, code) = run(&[
        &fixture("someapi123.har"),
        "compare",
        &fixture("someapi123.har"),
    ]);
    assert!(stdout.contains("max severity: none"));
    assert_eq!(code, 0);
}

#[test]
fn fail_on_high_gates_exit_code() {
    // Comparing a capture to itself yields no findings, so even --fail-on low exits 0.
    let (_stdout, code) = run(&[
        &fixture("someapi123.har"),
        "compare",
        &fixture("someapi123.har"),
        "--fail-on",
        "low",
    ]);
    assert_eq!(code, 0);
}

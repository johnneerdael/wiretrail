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
fn report_is_markdown() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "report"]);
    assert!(stdout.contains("# wiretrail report"));
    assert!(stdout.contains("## Subsystems"));
}

#[test]
fn curl_single_entry_redacts() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "curl", "e000000"]);
    assert!(stdout.contains("curl -X GET"));
    // no raw Authorization-style bearer secrets should leak by default
    assert!(!stdout.to_lowercase().contains("bearer "));
}

#[test]
fn curl_json_envelope() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "curl", "--json"]);
    assert!(stdout.contains("\"command\": \"curl\""));
    assert!(stdout.contains("\"commands\""));
}

#[test]
fn curl_unknown_id_exits_2() {
    let (_stdout, code) = run(&[&fixture("someapi123.har"), "curl", "e999999"]);
    assert_eq!(code, 2);
}

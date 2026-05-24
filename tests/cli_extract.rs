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
fn search_json_envelope() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "search", "api", "--json"]);
    assert!(stdout.contains("\"command\": \"search\""));
    assert!(stdout.contains("\"matches\""));
}

#[test]
fn extract_json_envelope() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "extract", "$.x", "--json"]);
    assert!(stdout.contains("\"command\": \"extract\""));
    assert!(stdout.contains("\"values\""));
}

#[test]
fn export_ndjson_is_line_oriented() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "export"]);
    assert!(stdout.lines().next().unwrap().starts_with('{'));
    assert!(stdout.contains("\"id\":\"e000000\""));
}

#[test]
fn export_csv_has_header() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "export", "--format", "csv"]);
    assert!(stdout.starts_with("id,offset_ms,"));
}

#[test]
fn invalid_regex_exits_2() {
    let (_stdout, code) = run(&[&fixture("someapi123.har"), "search", "(", "--regex"]);
    assert_eq!(code, 2);
}

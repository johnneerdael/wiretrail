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
fn duplicates_json_envelope() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "duplicates", "--json"]);
    assert!(stdout.contains("\"command\": \"duplicates\""));
    assert!(stdout.contains("\"groups\""));
}

#[test]
fn errors_text_header() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "errors"]);
    assert!(stdout.contains("== wiretrail errors =="));
}

#[test]
fn slowest_json_has_bottleneck_field() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "slowest", "--json"]);
    assert!(stdout.contains("\"command\": \"slowest\""));
    assert!(stdout.contains("\"bottleneck\""));
}

#[test]
fn show_entry_prints_detail() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "show-entry", "e000000"]);
    assert!(stdout.contains("== wiretrail entry e000000 =="));
}

#[test]
fn show_entry_unknown_id_exits_2() {
    let (_stdout, code) = run(&[&fixture("someapi123.har"), "show-entry", "e999999"]);
    assert_eq!(code, 2);
}

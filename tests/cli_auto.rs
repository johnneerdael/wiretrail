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
fn auto_json_envelope_has_summary_and_drilldowns() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "auto", "--json"]);
    assert!(stdout.contains("\"command\": \"auto\""));
    assert!(stdout.contains("\"summary\""));
    assert!(stdout.contains("\"drilldowns\""));
    assert!(stdout.contains("\"not_drilled\""));
}

#[test]
fn auto_text_includes_summary_header() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "auto"]);
    assert!(stdout.contains("== wiretrail summary =="));
}

#[test]
fn min_severity_high_drills_at_most_as_many_as_all() {
    let count_drilled = |args: &[&str]| -> usize {
        let (stdout, _) = run(args);
        stdout.matches("$ wiretrail ").count()
    };
    let high = count_drilled(&[&fixture("someapi123.har"), "auto", "--min-severity", "high"]);
    let all = count_drilled(&[&fixture("someapi123.har"), "auto", "--all"]);
    assert!(high <= all);
}

#[test]
fn summary_footer_is_present() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "summary", "--json"]);
    assert!(stdout.contains("\"next_commands\""));
}

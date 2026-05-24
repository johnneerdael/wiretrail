use std::process::Command;

const SENTINEL: &str = "FAKEKEY_a1b2c3d4e5f6g7h8";

fn fixture() -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/redaction_fixtures/secret_in_path.har")
        .to_string_lossy()
        .into_owned()
}

fn run(args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_wiretrail"))
        .args(args)
        .output()
        .expect("binary runs");
    let mut s = String::from_utf8_lossy(&out.stdout).into_owned();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    s
}

#[test]
fn no_command_leaks_path_secret_by_default() {
    let f = fixture();
    let commands: &[&[&str]] = &[
        &[&f, "summary"],
        &[&f, "duplicates"],
        &[&f, "redirects"],
        &[&f, "endpoints"],
        &[&f, "timeline"],
        &[&f, "report"],
        &[&f, "show-entry", "e000000"],
        &[&f, "curl", "e000000"],
    ];
    for args in commands {
        let out = run(args);
        assert!(
            !out.contains(SENTINEL),
            "command {:?} leaked the path secret:\n{out}",
            args
        );
    }
}

#[test]
fn unsafe_flag_reveals_secret_in_show_entry() {
    let f = fixture();
    let out = run(&[&f, "show-entry", "e000000", "--unsafe-include-secrets"]);
    assert!(
        out.contains(SENTINEL),
        "unsafe show-entry should reveal the secret:\n{out}"
    );
}

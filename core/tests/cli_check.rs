use flickfluent_core::cli;

fn fixture_path(name: &str) -> String {
    format!(
        "{}/tests/fixtures/{}.json",
        env!("CARGO_MANIFEST_DIR"),
        name
    )
}

fn run(fixture: &str, extra: &[&str]) -> (i32, String) {
    let mut args = vec![
        "--check-hardware".to_string(),
        "--fake-machine".to_string(),
        fixture_path(fixture),
    ];
    args.extend(extra.iter().map(|s| s.to_string()));
    let out = cli::run(&args).expect("cli flag recognised");
    (out.exit_code, out.stdout)
}

#[test]
fn blocked_machine_exits_10_with_json() {
    let (code, stdout) = run("low", &[]);
    assert_eq!(code, 10);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["mode"], "blocked");
    assert!(v["blockedMessage"]
        .as_str()
        .unwrap()
        .starts_with("Your computer is not compatible"));
}

#[test]
fn hybrid_exits_0() {
    let (code, stdout) = run("mid", &[]);
    assert_eq!(code, 0);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&stdout).unwrap()["mode"],
        "hybrid"
    );
}

#[test]
fn full_local_exits_0() {
    let (code, stdout) = run("high", &[]);
    assert_eq!(code, 0);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&stdout).unwrap()["mode"],
        "fullLocal"
    );
}

#[test]
fn message_flag_prints_only_the_message_when_blocked() {
    let (code, stdout) = run("low", &["--message"]);
    assert_eq!(code, 10);
    assert!(stdout.starts_with("Your computer is not compatible with this app."));
    assert!(!stdout.contains('{'));
}

#[test]
fn unrelated_args_are_ignored() {
    assert!(cli::run(&["--something-else".to_string()]).is_none());
}

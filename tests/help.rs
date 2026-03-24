use assert_cmd::cargo::cargo_bin_cmd;
use predicates::str::contains;

#[test]
fn serve_help_mentions_bind() {
    let mut cmd = cargo_bin_cmd!("prime-agent");
    cmd.arg("serve").arg("--help");
    cmd.assert().success().stdout(contains("--bind"));
}

#[test]
fn global_help_lists_data_dir() {
    let mut cmd = cargo_bin_cmd!("prime-agent");
    cmd.arg("--help");
    cmd.assert().success().stdout(contains("--data-dir"));
}

#[test]
fn help_subcommand_prints_usage() {
    let mut cmd = cargo_bin_cmd!("prime-agent");
    cmd.arg("help");
    cmd.assert().success().stdout(contains("--data-dir"));
}

#[test]
fn version_subcommand_prints_plain_version() {
    let mut cmd = cargo_bin_cmd!("prime-agent");
    cmd.arg("version");
    let out = cmd.assert().success().get_output().stdout.clone();
    let text = String::from_utf8_lossy(&out);
    assert!(
        !text.contains("prime-agent("),
        "version subcommand should not print the green banner line: {text:?}"
    );
    assert!(
        !text.contains('\u{001b}'),
        "version subcommand should not use ANSI escapes: {text:?}"
    );
    assert!(!text.trim().is_empty());
}

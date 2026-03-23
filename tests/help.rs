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

use assert_cmd::cargo::cargo_bin_cmd;
use predicates::str::contains as contains_text;
use std::fs;
use tempfile::TempDir;

#[test]
fn global_help_lists_clear_subcommand() {
    let mut cmd = cargo_bin_cmd!("prime-agent");
    cmd.arg("--help");
    cmd.assert().success().stdout(contains_text("clear"));
}

#[test]
fn clear_removes_pipeline_runs_directory() {
    let temp = TempDir::new().expect("temp");
    let pipelines = temp.path().join(".prime-agent/pipelines");
    fs::create_dir_all(pipelines.join("fake-run")).expect("run dir");
    fs::write(pipelines.join("fake-run/meta.json"), "{}\n").expect("meta");

    let mut cmd = cargo_bin_cmd!("prime-agent");
    cmd.current_dir(temp.path())
        .env("XDG_CONFIG_HOME", temp.path().join("config"))
        .arg("clear");
    cmd.assert().success();
    assert!(
        !temp.path().join(".prime-agent/pipelines").exists(),
        "clear should remove .prime-agent/pipelines"
    );
}

#[test]
fn clear_succeeds_when_pipelines_dir_absent() {
    let temp = TempDir::new().expect("temp");
    fs::create_dir_all(temp.path().join(".prime-agent")).expect("dot dir");
    assert!(
        !temp.path().join(".prime-agent/pipelines").exists(),
        "precondition: no pipelines dir"
    );

    let mut cmd = cargo_bin_cmd!("prime-agent");
    cmd.current_dir(temp.path())
        .env("XDG_CONFIG_HOME", temp.path().join("config"))
        .arg("clear");
    cmd.assert().success();
}

#[test]
fn clear_removes_pipelines_but_preserves_dot_prime_agent_config() {
    let temp = TempDir::new().expect("temp");
    let dot = temp.path().join(".prime-agent");
    fs::create_dir_all(dot.join("pipelines/stale-run")).expect("pipelines");
    fs::write(dot.join("pipelines/stale-run/meta.json"), "{}\n").expect("meta");
    fs::write(
        dot.join("config.json"),
        "{\"model\":\"m\",\"clirunner\":\"cursor-agent\"}\n",
    )
    .expect("config");

    let mut cmd = cargo_bin_cmd!("prime-agent");
    cmd.current_dir(temp.path())
        .env("XDG_CONFIG_HOME", temp.path().join("config"))
        .arg("clear");
    cmd.assert().success();

    assert!(!dot.join("pipelines").exists());
    let cfg = fs::read_to_string(dot.join("config.json")).expect("read config");
    assert!(
        cfg.contains("cursor-agent"),
        "clear must not remove .prime-agent/config.json: {cfg:?}"
    );
}

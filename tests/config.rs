use assert_cmd::cargo::cargo_bin_cmd;
use predicates::str::contains;
use std::fs;
use tempfile::TempDir;

#[test]
fn config_set_creates_file_and_get_reads_value() {
    let temp = TempDir::new().expect("temp dir");
    let config_home = temp.path().join("config");
    let mut cmd = cargo_bin_cmd!("prime-agent");
    cmd.current_dir(temp.path())
        .env("XDG_CONFIG_HOME", &config_home)
        .arg("config")
        .arg("set")
        .arg("skills-dir")
        .arg("/tmp/example");
    cmd.assert()
        .success()
        .stdout(contains("skills-dir=/tmp/example (updated)\n"));

    let mut get_cmd = cargo_bin_cmd!("prime-agent");
    get_cmd
        .current_dir(temp.path())
        .env("XDG_CONFIG_HOME", &config_home)
        .arg("config")
        .arg("get")
        .arg("skills-dir");
    get_cmd
        .assert()
        .success()
        .stdout(contains("/tmp/example\n"));
}

#[test]
fn config_list_prints_all_values() {
    let temp = TempDir::new().expect("temp dir");
    let config_home = temp.path().join("config");

    let mut set_cmd = cargo_bin_cmd!("prime-agent");
    set_cmd
        .current_dir(temp.path())
        .env("XDG_CONFIG_HOME", &config_home)
        .arg("config")
        .arg("set")
        .arg("skills-dir")
        .arg("/tmp/skills");
    set_cmd.assert().success();

    let mut set_other = cargo_bin_cmd!("prime-agent");
    set_other
        .current_dir(temp.path())
        .env("XDG_CONFIG_HOME", &config_home)
        .arg("config")
        .arg("set")
        .arg("owner")
        .arg("prime");
    set_other
        .assert()
        .success()
        .stdout(contains("owner=prime (updated)\n"));

    let mut list_cmd = cargo_bin_cmd!("prime-agent");
    list_cmd
        .current_dir(temp.path())
        .env("XDG_CONFIG_HOME", &config_home)
        .arg("config");
    list_cmd
        .assert()
        .success()
        .stdout(contains("Required:\n"))
        .stdout(contains("skills-dir=/tmp/skills\n"))
        .stdout(contains("Optional:\n"))
        .stdout(contains("owner=prime\n"));
}

#[test]
fn config_override_skills_dir_allows_missing_config_file() {
    let temp = TempDir::new().expect("temp dir");
    let source = temp.path().join("source.md");
    fs::write(&source, "Override content\n").expect("source");

    let home = temp.path().join("home");
    fs::create_dir_all(&home).expect("home dir");
    let expected_path = home.join("override-skills/alpha/SKILL.md");

    let mut cmd = cargo_bin_cmd!("prime-agent");
    cmd.current_dir(temp.path())
        .env("XDG_CONFIG_HOME", temp.path().join("missing-config"))
        .env("HOME", &home)
        .arg("--config")
        .arg("skills-dir:~/override-skills")
        .arg("set")
        .arg("alpha")
        .arg(&source);
    cmd.assert().success();

    assert!(expected_path.exists());
}

#[test]
fn config_get_creates_missing_file() {
    let temp = TempDir::new().expect("temp dir");
    let config_home = temp.path().join("config");
    let config_path = config_home.join("prime-agent").join("config");

    let mut get_cmd = cargo_bin_cmd!("prime-agent");
    get_cmd
        .current_dir(temp.path())
        .env("XDG_CONFIG_HOME", &config_home)
        .arg("config")
        .arg("get")
        .arg("missing");
    get_cmd.assert().failure();

    assert!(config_path.exists());
}

#[test]
fn config_set_skills_dir_relative_expands_to_cwd() {
    let temp = TempDir::new().expect("temp dir");
    let config_home = temp.path().join("config");

    let mut cmd = cargo_bin_cmd!("prime-agent");
    cmd.current_dir(temp.path())
        .env("XDG_CONFIG_HOME", &config_home)
        .arg("config")
        .arg("set")
        .arg("skills-dir")
        .arg(".");
    cmd.assert().success();

    let mut list_cmd = cargo_bin_cmd!("prime-agent");
    list_cmd
        .current_dir(temp.path())
        .env("XDG_CONFIG_HOME", &config_home)
        .arg("config");
    list_cmd
        .assert()
        .success()
        .stdout(contains(format!("skills-dir={}\n", temp.path().display())));
}

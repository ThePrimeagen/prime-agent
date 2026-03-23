use assert_cmd::Command;
use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use predicates::str::contains as contains_text;
use serde_json::json;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use tempfile::TempDir;

fn cmd_with_skills_dir(temp: &TempDir, skills_dir: &Path) -> Command {
    let mut cmd = cargo_bin_cmd!("prime-agent");
    cmd.current_dir(temp.path())
        .env("XDG_CONFIG_HOME", temp.path().join("config"))
        .arg("--skills-dir")
        .arg(skills_dir);
    cmd
}

fn default_agents_path(temp: &TempDir) -> PathBuf {
    temp.path().join("AGENTS.md")
}

fn write_config(temp: &TempDir, skills_dir: &Path) -> PathBuf {
    let config_dir = temp.path().join("config/prime-agent");
    fs::create_dir_all(&config_dir).expect("config dir");
    let config_path = config_dir.join("config");
    let config = format!("{{\n  \"skills-dir\": \"{}\"\n}}\n", skills_dir.display());
    fs::write(&config_path, config).expect("write config");
    config_path
}

fn run_git(dir: &Path, args: &[&str]) {
    let status = ProcessCommand::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .status()
        .expect("git command");
    assert!(status.success());
}

fn git_output(dir: &Path, args: &[&str]) -> String {
    let output = ProcessCommand::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("git output");
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn get_builds_agents_from_skills() {
    let temp = TempDir::new().expect("temp dir");
    let skills_dir = temp.path().join("skills");
    write_config(&temp, &skills_dir);
    fs::create_dir_all(&skills_dir).expect("skills dir");
    fs::create_dir_all(skills_dir.join("alpha")).expect("alpha dir");
    fs::create_dir_all(skills_dir.join("beta")).expect("beta dir");
    fs::write(skills_dir.join("alpha/SKILL.md"), "Alpha instructions\n").expect("alpha");
    fs::write(skills_dir.join("beta/SKILL.md"), "Beta instructions\n").expect("beta");

    let mut cmd = cmd_with_skills_dir(&temp, &skills_dir);
    cmd.arg("get").arg("alpha,beta");
    cmd.assert().success();

    let agents = fs::read_to_string(default_agents_path(&temp)).expect("AGENTS");
    assert!(agents.contains("<!-- prime-agent(Start alpha) -->"));
    assert!(agents.contains("## alpha"));
    assert!(agents.contains("Alpha instructions"));
    assert!(agents.contains("<!-- prime-agent(End alpha) -->"));
    assert!(agents.contains("<!-- prime-agent(Start beta) -->"));
}

#[test]
fn set_writes_skill_file() {
    let temp = TempDir::new().expect("temp dir");
    let skills_dir = temp.path().join("skills");
    write_config(&temp, &skills_dir);
    let source = temp.path().join("source.md");
    fs::write(&source, "Skill content\n").expect("source");

    let mut cmd = cmd_with_skills_dir(&temp, &skills_dir);
    cmd.arg("set").arg("alpha").arg(&source);
    cmd.assert().success();

    let skill = fs::read_to_string(skills_dir.join("alpha/SKILL.md")).expect("skill");
    assert!(skill.contains("Skill content"));
}

#[test]
fn sync_updates_skill_from_agents_section() {
    let temp = TempDir::new().expect("temp dir");
    let skills_dir = temp.path().join("skills");
    write_config(&temp, &skills_dir);
    fs::create_dir_all(&skills_dir).expect("skills dir");
    fs::create_dir_all(skills_dir.join("alpha")).expect("alpha dir");
    fs::write(skills_dir.join("alpha/SKILL.md"), "Old content\n").expect("skill");
    let agents = [
        "<!-- prime-agent(Start alpha) -->",
        "## alpha",
        "Updated content",
        "<!-- prime-agent(End alpha) -->",
        "",
    ]
    .join("\n");
    fs::write(default_agents_path(&temp), agents).expect("agents");

    let mut cmd = cmd_with_skills_dir(&temp, &skills_dir);
    cmd.arg("sync").write_stdin("a\n");
    cmd.assert().success();

    let skill = fs::read_to_string(skills_dir.join("alpha/SKILL.md")).expect("skill");
    assert!(skill.contains("Updated content"));
}

#[test]
fn sync_fails_on_broken_markers() {
    let temp = TempDir::new().expect("temp dir");
    let skills_dir = temp.path().join("skills");
    write_config(&temp, &skills_dir);
    let agents = [
        "<!-- prime-agent(Start alpha) -->",
        "## alpha",
        "Broken section",
        "<!-- prime-agent(End beta) -->",
        "",
    ]
    .join("\n");
    fs::write(default_agents_path(&temp), agents).expect("agents");

    let mut cmd = cmd_with_skills_dir(&temp, &skills_dir);
    cmd.arg("sync");
    cmd.assert().failure();
}

#[test]
fn personal_instructions_are_preserved() {
    let temp = TempDir::new().expect("temp dir");
    let skills_dir = temp.path().join("skills");
    write_config(&temp, &skills_dir);
    fs::create_dir_all(&skills_dir).expect("skills dir");
    fs::create_dir_all(skills_dir.join("alpha")).expect("alpha dir");
    fs::write(skills_dir.join("alpha/SKILL.md"), "Skill content\n").expect("skill");
    let agents = [
        "# My Personal Notes",
        "Use this workspace carefully.",
        "",
        "<!-- prime-agent(Start beta) -->",
        "## beta",
        "Beta rules",
        "<!-- prime-agent(End beta) -->",
        "",
        "Trailing notes stay here.",
        "",
    ]
    .join("\n");
    fs::write(default_agents_path(&temp), agents).expect("agents");

    let mut cmd = cmd_with_skills_dir(&temp, &skills_dir);
    cmd.arg("sync");
    cmd.assert().success();

    let updated = fs::read_to_string(default_agents_path(&temp)).expect("agents");
    assert!(updated.contains("My Personal Notes"));
    assert!(updated.contains("Trailing notes stay here."));
    assert!(!updated.contains("<!-- prime-agent(Start alpha) -->"));
}

#[test]
fn sync_does_not_add_missing_skills_to_agents() {
    let temp = TempDir::new().expect("temp dir");
    let skills_dir = temp.path().join("skills");
    write_config(&temp, &skills_dir);
    fs::create_dir_all(&skills_dir).expect("skills dir");
    fs::create_dir_all(skills_dir.join("alpha")).expect("alpha dir");
    fs::write(skills_dir.join("alpha/SKILL.md"), "From skill\n").expect("skill");
    fs::write(default_agents_path(&temp), "# Notes\n").expect("agents");

    let mut cmd = cmd_with_skills_dir(&temp, &skills_dir);
    cmd.arg("sync");
    cmd.assert().success();

    let agents = fs::read_to_string(default_agents_path(&temp)).expect("agents");
    assert_eq!(agents, "# Notes\n");
}

#[test]
fn delete_removes_only_agents_section() {
    let temp = TempDir::new().expect("temp dir");
    let skills_dir = temp.path().join("skills");
    write_config(&temp, &skills_dir);
    fs::create_dir_all(&skills_dir).expect("skills dir");
    fs::create_dir_all(skills_dir.join("alpha")).expect("alpha dir");
    fs::write(skills_dir.join("alpha/SKILL.md"), "Skill content\n").expect("skill");
    let agents = [
        "<!-- prime-agent(Start alpha) -->",
        "## alpha",
        "Agent rules",
        "<!-- prime-agent(End alpha) -->",
        "",
    ]
    .join("\n");
    fs::write(default_agents_path(&temp), agents).expect("agents");

    let mut cmd = cmd_with_skills_dir(&temp, &skills_dir);
    cmd.arg("delete").arg("alpha");
    cmd.assert().success();

    let updated = fs::read_to_string(default_agents_path(&temp)).expect("agents");
    assert!(!updated.contains("prime-agent(Start alpha)"));
    assert!(skills_dir.join("alpha/SKILL.md").exists());
}

#[test]
fn delete_globally_removes_agents_and_skill_file() {
    let temp = TempDir::new().expect("temp dir");
    let skills_dir = temp.path().join("skills");
    write_config(&temp, &skills_dir);
    fs::create_dir_all(&skills_dir).expect("skills dir");
    fs::create_dir_all(skills_dir.join("alpha")).expect("alpha dir");
    fs::write(skills_dir.join("alpha/SKILL.md"), "Skill content\n").expect("skill");
    let agents = [
        "<!-- prime-agent(Start alpha) -->",
        "## alpha",
        "Agent rules",
        "<!-- prime-agent(End alpha) -->",
        "",
    ]
    .join("\n");
    fs::write(default_agents_path(&temp), agents).expect("agents");

    let mut cmd = cmd_with_skills_dir(&temp, &skills_dir);
    cmd.arg("delete-globally").arg("alpha");
    cmd.assert().success();

    let updated = fs::read_to_string(default_agents_path(&temp)).expect("agents");
    assert!(!updated.contains("prime-agent(Start alpha)"));
    assert!(!skills_dir.join("alpha/SKILL.md").exists());
}

#[test]
fn sync_prefers_skill_update_when_selected() {
    let temp = TempDir::new().expect("temp dir");
    let skills_dir = temp.path().join("skills");
    write_config(&temp, &skills_dir);
    fs::create_dir_all(&skills_dir).expect("skills dir");
    fs::create_dir_all(skills_dir.join("alpha")).expect("alpha dir");
    fs::write(skills_dir.join("alpha/SKILL.md"), "Skill version\n").expect("skill");
    let agents = [
        "<!-- prime-agent(Start alpha) -->",
        "## alpha",
        "Agents version",
        "<!-- prime-agent(End alpha) -->",
        "",
    ]
    .join("\n");
    fs::write(default_agents_path(&temp), agents).expect("agents");

    let mut cmd = cmd_with_skills_dir(&temp, &skills_dir);
    cmd.arg("sync").write_stdin("s\n");
    cmd.assert().success();

    let updated = fs::read_to_string(default_agents_path(&temp)).expect("agents");
    assert!(updated.contains("Skill version"));
}

#[test]
fn sync_prefers_agents_update_when_selected() {
    let temp = TempDir::new().expect("temp dir");
    let skills_dir = temp.path().join("skills");
    write_config(&temp, &skills_dir);
    fs::create_dir_all(&skills_dir).expect("skills dir");
    fs::create_dir_all(skills_dir.join("alpha")).expect("alpha dir");
    fs::write(skills_dir.join("alpha/SKILL.md"), "Skill version\n").expect("skill");
    let agents = [
        "<!-- prime-agent(Start alpha) -->",
        "## alpha",
        "Agents version",
        "<!-- prime-agent(End alpha) -->",
        "",
    ]
    .join("\n");
    fs::write(default_agents_path(&temp), agents).expect("agents");

    let mut cmd = cmd_with_skills_dir(&temp, &skills_dir);
    cmd.arg("sync").write_stdin("a\n");
    cmd.assert().success();

    let skill = fs::read_to_string(skills_dir.join("alpha/SKILL.md")).expect("skill");
    assert!(skill.contains("Agents version"));
}

#[test]
fn explicit_skills_dir_flag_sets_target() {
    let temp = TempDir::new().expect("temp dir");
    let skills_dir = temp.path().join("custom_skills");
    fs::create_dir_all(&skills_dir).expect("custom_skills");
    let source = temp.path().join("source.md");
    fs::write(&source, "Env content\n").expect("source");

    let mut cmd = cargo_bin_cmd!("prime-agent");
    cmd.current_dir(temp.path())
        .env("XDG_CONFIG_HOME", temp.path().join("config"))
        .args([
            "--skills-dir",
            skills_dir.to_str().expect("utf8"),
            "set",
            "alpha",
        ])
        .arg(&source);
    cmd.assert().success();

    let skill = fs::read_to_string(skills_dir.join("alpha/SKILL.md")).expect("skill");
    assert!(skill.contains("Env content"));
}

#[test]
fn set_uses_cwd_skills_without_skills_flag() {
    let temp = TempDir::new().expect("temp dir");
    let skills_dir = temp.path().join("skills");
    let source = temp.path().join("source.md");
    fs::write(&source, "Config content\n").expect("source");

    let mut cmd = cargo_bin_cmd!("prime-agent");
    cmd.current_dir(temp.path())
        .env("XDG_CONFIG_HOME", temp.path().join("config"))
        .arg("set")
        .arg("alpha")
        .arg(&source);
    cmd.assert().success();

    let skill = fs::read_to_string(skills_dir.join("alpha/SKILL.md")).expect("skill");
    assert!(skill.contains("Config content"));
}

#[test]
fn set_writes_skill_under_cwd_skills_without_config_file() {
    let temp = TempDir::new().expect("temp dir");
    let source = temp.path().join("source.md");
    fs::write(&source, "Config content\n").expect("source");

    let mut cmd = cargo_bin_cmd!("prime-agent");
    cmd.current_dir(temp.path())
        .env("XDG_CONFIG_HOME", temp.path().join("missing-config"))
        .arg("set")
        .arg("alpha")
        .arg(&source);
    cmd.assert().success();

    let expected = temp.path().join("skills/alpha/SKILL.md");
    let skill = fs::read_to_string(expected).expect("skill");
    assert!(skill.contains("Config content"));
}

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
fn list_outputs_skill_names() {
    let temp = TempDir::new().expect("temp dir");
    let skills_dir = temp.path().join("skills");
    write_config(&temp, &skills_dir);
    fs::create_dir_all(&skills_dir).expect("skills dir");
    fs::create_dir_all(skills_dir.join("alpha")).expect("alpha dir");
    fs::create_dir_all(skills_dir.join("beta")).expect("beta dir");
    fs::write(skills_dir.join("alpha/SKILL.md"), "Alpha\n").expect("alpha");
    fs::write(skills_dir.join("beta/SKILL.md"), "Beta\n").expect("beta");

    let mut cmd = cmd_with_skills_dir(&temp, &skills_dir);
    cmd.arg("list");
    cmd.assert().success().stdout(contains("alpha\n\nbeta\n"));
}

#[test]
fn list_marks_out_of_sync_skills() {
    let temp = TempDir::new().expect("temp dir");
    let skills_dir = temp.path().join("skills");
    write_config(&temp, &skills_dir);
    fs::create_dir_all(skills_dir.join("alpha")).expect("alpha dir");
    fs::write(skills_dir.join("alpha/SKILL.md"), "Alpha\n").expect("alpha");

    let agents = [
        "<!-- prime-agent(Start alpha) -->",
        "## alpha",
        "Changed",
        "<!-- prime-agent(End alpha) -->",
        "",
    ]
    .join("\n");
    fs::write(default_agents_path(&temp), agents).expect("agents");

    let mut cmd = cmd_with_skills_dir(&temp, &skills_dir);
    cmd.arg("local");
    cmd.assert()
        .success()
        .stdout(contains("alpha (out of sync: conflict)\n"));
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

#[test]
fn sync_commits_skills_repo() {
    let temp = TempDir::new().expect("temp dir");
    let skills_dir = temp.path().join("skills");
    write_config(&temp, &skills_dir);
    fs::create_dir_all(skills_dir.join("alpha")).expect("alpha dir");
    fs::write(skills_dir.join("alpha/SKILL.md"), "Initial\n").expect("skill");

    run_git(&skills_dir, &["init"]);
    run_git(&skills_dir, &["config", "user.email", "test@example.com"]);
    run_git(&skills_dir, &["config", "user.name", "Test"]);
    run_git(&skills_dir, &["add", "-A"]);
    run_git(&skills_dir, &["commit", "-m", "Initial"]);

    let agents = [
        "<!-- prime-agent(Start alpha) -->",
        "## alpha",
        "Updated content",
        "<!-- prime-agent(End alpha) -->",
        "",
    ]
    .join("\n");
    fs::write(default_agents_path(&temp), agents).expect("agents");

    let mut cmd = cmd_with_skills_dir(&temp, &skills_dir);
    cmd.arg("sync").write_stdin("a\n");
    cmd.assert().success();

    let count = git_output(&skills_dir, &["rev-list", "--count", "HEAD"]);
    assert_eq!(count.trim(), "2");
}

#[test]
fn list_with_fragment_outputs_single_line() {
    let temp = TempDir::new().expect("temp dir");
    let skills_dir = temp.path().join("skills");
    write_config(&temp, &skills_dir);
    fs::create_dir_all(skills_dir.join("zephyr-a")).expect("zephyr-a dir");
    fs::create_dir_all(skills_dir.join("zephyr-b")).expect("zephyr-b dir");
    fs::create_dir_all(skills_dir.join("other")).expect("other dir");
    fs::write(skills_dir.join("zephyr-a/SKILL.md"), "A\n").expect("skill");
    fs::write(skills_dir.join("zephyr-b/SKILL.md"), "B\n").expect("skill");
    fs::write(skills_dir.join("other/SKILL.md"), "C\n").expect("skill");

    let mut cmd = cmd_with_skills_dir(&temp, &skills_dir);
    cmd.arg("list").arg("zephyr");
    cmd.assert()
        .success()
        .stdout(contains("zephyr-a zephyr-b\n"));
}

#[test]
fn local_marks_out_of_sync_by_source() {
    let temp = TempDir::new().expect("temp dir");
    let skills_dir = temp.path().join("skills");
    write_config(&temp, &skills_dir);
    fs::create_dir_all(skills_dir.join("alpha")).expect("alpha dir");
    fs::write(skills_dir.join("alpha/SKILL.md"), "Alpha\n").expect("skill");
    let agents = [
        "<!-- prime-agent(Start alpha) -->",
        "## alpha",
        "Remote",
        "<!-- prime-agent(End alpha) -->",
        "",
    ]
    .join("\n");
    fs::write(default_agents_path(&temp), agents).expect("agents");

    let mut cmd = cmd_with_skills_dir(&temp, &skills_dir);
    cmd.arg("local");
    cmd.assert()
        .success()
        .stdout(contains("alpha (out of sync: conflict)\n"));
}

#[test]
fn local_without_agents_does_not_mark_out_of_sync() {
    let temp = TempDir::new().expect("temp dir");
    let skills_dir = temp.path().join("skills");
    write_config(&temp, &skills_dir);
    fs::create_dir_all(skills_dir.join("alpha")).expect("alpha dir");
    fs::write(skills_dir.join("alpha/SKILL.md"), "Alpha\n").expect("skill");

    let mut cmd = cmd_with_skills_dir(&temp, &skills_dir);
    cmd.arg("local");
    cmd.assert()
        .success()
        .stdout(contains_text("alpha").not())
        .stdout(contains_text("out of sync").not());
}

#[test]
fn local_with_empty_agents_does_not_mark_out_of_sync() {
    let temp = TempDir::new().expect("temp dir");
    let skills_dir = temp.path().join("skills");
    write_config(&temp, &skills_dir);
    fs::create_dir_all(skills_dir.join("alpha")).expect("alpha dir");
    fs::write(skills_dir.join("alpha/SKILL.md"), "Alpha\n").expect("skill");
    fs::write(default_agents_path(&temp), "").expect("agents");

    let mut cmd = cmd_with_skills_dir(&temp, &skills_dir);
    cmd.arg("local");
    cmd.assert()
        .success()
        .stdout(contains_text("alpha").not())
        .stdout(contains_text("out of sync").not());
}

#[test]
fn sync_remote_commits_and_pulls() {
    let temp = TempDir::new().expect("temp dir");
    let skills_dir = temp.path().join("skills");
    let remote_dir = temp.path().join("remote.git");
    write_config(&temp, &skills_dir);

    fs::create_dir_all(skills_dir.join("alpha")).expect("alpha dir");
    fs::write(skills_dir.join("alpha/SKILL.md"), "Initial\n").expect("skill");

    run_git(&skills_dir, &["init"]);
    run_git(&skills_dir, &["config", "user.email", "test@example.com"]);
    run_git(&skills_dir, &["config", "user.name", "Test"]);
    run_git(&skills_dir, &["add", "-A"]);
    run_git(&skills_dir, &["commit", "-m", "Initial"]);

    run_git(
        temp.path(),
        &["init", "--bare", remote_dir.to_str().expect("remote")],
    );
    run_git(
        &skills_dir,
        &[
            "remote",
            "add",
            "origin",
            remote_dir.to_str().expect("remote"),
        ],
    );
    run_git(&skills_dir, &["push", "-u", "origin", "HEAD"]);

    let agents = [
        "<!-- prime-agent(Start alpha) -->",
        "## alpha",
        "Updated content",
        "<!-- prime-agent(End alpha) -->",
        "",
    ]
    .join("\n");
    fs::write(default_agents_path(&temp), agents).expect("agents");

    let mut cmd = cmd_with_skills_dir(&temp, &skills_dir);
    cmd.arg("sync-remote").write_stdin("a\n");
    cmd.assert().success();
}

#[test]
fn get_errors_when_no_skills_provided() {
    let temp = TempDir::new().expect("temp dir");
    let skills_dir = temp.path().join("skills");
    write_config(&temp, &skills_dir);
    fs::create_dir_all(&skills_dir).expect("skills dir");

    let mut cmd = cmd_with_skills_dir(&temp, &skills_dir);
    cmd.arg("get");
    cmd.assert()
        .failure()
        .stderr(contains("no skills provided"));
}

#[test]
fn local_marks_out_of_sync_remote_when_skill_missing_on_disk() {
    let temp = TempDir::new().expect("temp dir");
    let skills_dir = temp.path().join("skills");
    write_config(&temp, &skills_dir);
    fs::create_dir_all(&skills_dir).expect("skills dir");

    let agents = [
        "<!-- prime-agent(Start alpha) -->",
        "## alpha",
        "Only in agents",
        "<!-- prime-agent(End alpha) -->",
        "",
    ]
    .join("\n");
    fs::write(default_agents_path(&temp), agents).expect("agents");

    let mut cmd = cmd_with_skills_dir(&temp, &skills_dir);
    cmd.arg("local");
    cmd.assert()
        .success()
        .stdout(contains("alpha (out of sync: remote)\n"));
}

#[test]
fn sync_without_agents_md_commits_skills_repo_when_dirty() {
    let temp = TempDir::new().expect("temp dir");
    let skills_dir = temp.path().join("skills");
    write_config(&temp, &skills_dir);
    fs::create_dir_all(skills_dir.join("alpha")).expect("alpha dir");
    fs::write(skills_dir.join("alpha/SKILL.md"), "Orphan skill\n").expect("skill");

    run_git(&skills_dir, &["init"]);
    run_git(&skills_dir, &["config", "user.email", "test@example.com"]);
    run_git(&skills_dir, &["config", "user.name", "Test"]);
    run_git(&skills_dir, &["add", "-A"]);
    run_git(&skills_dir, &["commit", "-m", "Initial"]);

    assert!(!default_agents_path(&temp).exists());

    fs::write(skills_dir.join("alpha/SKILL.md"), "Dirty working tree\n").expect("skill");

    let mut cmd = cmd_with_skills_dir(&temp, &skills_dir);
    cmd.arg("sync");
    cmd.assert().success();

    assert!(!default_agents_path(&temp).exists());

    let count = git_output(&skills_dir, &["rev-list", "--count", "HEAD"]);
    assert_eq!(count.trim(), "2");
}

#[test]
fn sync_resolves_multi_hunk_conflict_via_stdin() {
    let temp = TempDir::new().expect("temp dir");
    let skills_dir = temp.path().join("skills");
    write_config(&temp, &skills_dir);
    fs::create_dir_all(skills_dir.join("alpha")).expect("alpha dir");
    let middle = (2..20)
        .map(|i| format!("m{i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let skill_body = format!("L1\n{middle}\nL20\n");
    fs::write(skills_dir.join("alpha/SKILL.md"), &skill_body).expect("skill");
    let agents_body = format!("ONE\n{middle}\nTWENTY\n");
    let agents = format!(
        "<!-- prime-agent(Start alpha) -->\n## alpha\n{agents_body}<!-- prime-agent(End alpha) -->\n\n",
    );
    fs::write(default_agents_path(&temp), agents).expect("agents");

    let mut cmd = cmd_with_skills_dir(&temp, &skills_dir);
    cmd.arg("sync").write_stdin("s\na\n");
    cmd.assert().success();

    let skill = fs::read_to_string(skills_dir.join("alpha/SKILL.md")).expect("skill");
    assert!(skill.starts_with("L1\n"));
    assert!(skill.contains("TWENTY"));
    assert!(!skill.contains("ONE"));

    let updated = fs::read_to_string(default_agents_path(&temp)).expect("agents");
    assert!(updated.contains("L1\n"));
    assert!(updated.contains("TWENTY"));
}

fn write_dot_prime_agent_config(temp: &TempDir, model: &str, clirunner: &str) {
    write_dot_prime_agent_config_yolo(temp, model, clirunner, false);
}

fn write_dot_prime_agent_config_yolo(temp: &TempDir, model: &str, clirunner: &str, yolo: bool) {
    let d = temp.path().join(".prime-agent");
    fs::create_dir_all(&d).expect("dot dir");
    let j = if yolo {
        json!({ "model": model, "clirunner": clirunner, "yolo": true })
    } else {
        json!({ "model": model, "clirunner": clirunner, "yolo": false })
    };
    fs::write(
        d.join("config.json"),
        format!("{}\n", serde_json::to_string_pretty(&j).expect("json")),
    )
    .expect("dot config");
}

fn chmod_x(path: &Path) {
    #[cfg(unix)]
    {
        let mut perms = fs::metadata(path).expect("meta").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("chmod");
    }
}

fn write_pipeline(data_dir: &Path, name: &str, steps: &str) {
    let dir = data_dir.join("pipelines").join(name);
    fs::create_dir_all(&dir).expect("pipeline dir");
    fs::write(dir.join("pipeline.json"), steps).expect("pipeline.json");
}

/// Run artifacts live under `cwd/.prime-agent/pipelines/<adj-noun-slug>/`; find by `meta.json` `pipeline` field.
fn pipeline_artifact_dir_for(cwd: &Path, pipeline_name: &str) -> PathBuf {
    let root = cwd.join(".prime-agent/pipelines");
    let rd = fs::read_dir(&root).unwrap_or_else(|e| panic!("read_dir {:?}: {}", root, e));
    for entry in rd {
        let p = entry.expect("entry").path();
        if !p.is_dir() || !p.join("meta.json").is_file() {
            continue;
        }
        let raw = fs::read_to_string(p.join("meta.json")).expect("meta");
        let v: serde_json::Value = serde_json::from_str(&raw).expect("parse meta");
        if v.get("pipeline").and_then(|x| x.as_str()) == Some(pipeline_name) {
            return p;
        }
    }
    panic!("no run dir for pipeline '{pipeline_name}' under {:?}", root);
}

/// Everything after the first line of stdout (the `prime-agent(version)` banner).
fn stdout_after_version_line(stdout: &[u8]) -> String {
    let text = String::from_utf8_lossy(stdout);
    match text.split_once('\n') {
        Some((_, rest)) => rest.to_string(),
        None => String::new(),
    }
}

fn pipelines_cmd(temp: &TempDir, data_dir: &Path, skills_dir: &Path, bin_dir: &Path) -> Command {
    let path_var = format!(
        "{}:{}",
        bin_dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut cmd = cargo_bin_cmd!("prime-agent");
    cmd.current_dir(temp.path())
        .env("PATH", path_var)
        .env("PRIME_AGENT_NO_TUI", "1")
        .env("XDG_CONFIG_HOME", temp.path().join("config"))
        .arg("--data-dir")
        .arg(data_dir)
        .arg("--skills-dir")
        .arg(skills_dir);
    cmd
}

#[test]
fn pipelines_default_no_subcommand_lists_two_sorted() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_pipeline(
        &data_dir,
        "beta",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    write_pipeline(
        &data_dir,
        "alpha",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.arg("pipelines");
    let out = cmd.assert().success().get_output().stdout.clone();
    assert_eq!(stdout_after_version_line(&out), "alpha\n\nbeta\n");
}

#[test]
fn pipelines_default_no_subcommand_lists_one() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_pipeline(
        &data_dir,
        "solo",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.arg("pipelines");
    let out = cmd.assert().success().get_output().stdout.clone();
    assert_eq!(stdout_after_version_line(&out), "solo\n");
}

#[test]
fn pipelines_default_no_subcommand_empty_stderr() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    fs::create_dir_all(&data_dir).expect("data");
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.arg("pipelines");
    let out = cmd.assert().success().get_output().clone();
    assert_eq!(stdout_after_version_line(&out.stdout), "");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("No pipelines found."));
}

#[test]
fn pipelines_default_no_subcommand_ignores_incomplete_dir() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    fs::create_dir_all(data_dir.join("pipelines/incomplete")).expect("incomplete");
    write_pipeline(
        &data_dir,
        "good",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.arg("pipelines");
    let out = cmd.assert().success().get_output().stdout.clone();
    assert_eq!(stdout_after_version_line(&out), "good\n");
}

#[test]
fn pipelines_default_no_subcommand_respects_data_dir_flag() {
    let temp = TempDir::new().expect("temp");
    let right = temp.path().join("right");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_pipeline(
        &right,
        "only",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");

    let mut cmd = pipelines_cmd(&temp, &right, &skills_dir, &bin);
    cmd.arg("pipelines");
    let out = cmd.assert().success().get_output().stdout.clone();
    assert_eq!(stdout_after_version_line(&out), "only\n");
}

#[test]
fn pipelines_help_mentions_default_behavior() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["pipelines", "--help"]);
    cmd.assert()
        .success()
        .stdout(contains_text("run"))
        .stdout(contains_text("pipeline"));
}

#[test]
fn pipelines_run_errors_when_neither_prompt_nor_file() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(&mock, "#!/bin/sh\necho '{\"text\":\"x\"}'\n").expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["pipelines", "run", "pl"]);
    cmd.assert().failure();
}

#[test]
fn pipelines_run_errors_when_both_prompt_and_file() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(&mock, "#!/bin/sh\necho '{\"text\":\"x\"}'\n").expect("mock");
    chmod_x(&mock);
    let f = temp.path().join("prompt.txt");
    fs::write(&f, "hi").expect("f");

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["pipelines", "run", "pl", "--prompt", "a", "--file"])
        .arg(&f);
    cmd.assert().failure();
}

#[test]
fn pipelines_run_errors_when_dot_prime_agent_config_missing() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(&mock, "#!/bin/sh\necho '{\"text\":\"x\"}'\n").expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["pipelines", "run", "pl", "--prompt", "x"]);
    cmd.assert().failure();
}

#[test]
fn pipelines_run_errors_on_unsupported_clirunner() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "m", "other-cli");
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(&mock, "#!/bin/sh\necho '{\"text\":\"x\"}'\n").expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["pipelines", "run", "pl", "--prompt", "x"]);
    cmd.assert()
        .failure()
        .stderr(contains("unsupported clirunner"));
}

#[test]
fn pipelines_run_data_dir_skills_overrides_global_config_skills_dir() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("skill-issues");
    let skills_under_data = data_dir.join("skills");
    fs::create_dir_all(skills_under_data.join("attached-skill")).expect("skill dir");
    fs::write(skills_under_data.join("attached-skill/SKILL.md"), "x\n").expect("skill");

    let wrong_skills = temp.path().join("other-skills");
    fs::create_dir_all(&wrong_skills).expect("wrong");

    let config_home = temp.path().join("xdg_config");
    fs::create_dir_all(config_home.join("prime-agent")).expect("prime cfg dir");
    let cfg = config_home.join("prime-agent/config.json");
    let cfg_json = json!({
        "skills-dir": wrong_skills.to_string_lossy().to_string(),
    });
    fs::write(
        &cfg,
        format!("{}\n", serde_json::to_string_pretty(&cfg_json).unwrap()),
    )
    .expect("cfg");

    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":["attached-skill"]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(
        &mock,
        "#!/bin/sh\ncat >/dev/null\necho '{\"text\":\"ok\"}'\n",
    )
    .expect("mock");
    chmod_x(&mock);

    let path_var = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut cmd = cargo_bin_cmd!("prime-agent");
    cmd.current_dir(temp.path())
        .env("PATH", &path_var)
        .env("XDG_CONFIG_HOME", &config_home)
        .env("PRIME_AGENT_NO_TUI", "1")
        .args([
            "--data-dir",
            data_dir.to_str().expect("utf8 path"),
            "pipelines",
            "run",
            "pl",
            "--prompt",
            "u",
        ]);
    cmd.assert().success();
}

#[test]
fn pipelines_run_writes_stage_files_under_pipeline_dir() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "composer-2-fast", "cursor-agent");
    write_pipeline(
        &data_dir,
        "demo-pipe",
        r#"{"steps":[{"id":1,"title":"stepone","prompt":"doprompt","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(
        &mock,
        "#!/bin/sh\ncat >/dev/null\necho '{\"text\":\"out1\"}'\n",
    )
    .expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["pipelines", "run", "demo-pipe", "--prompt", "userhi"]);
    cmd.assert().success();

    let one = pipeline_artifact_dir_for(temp.path(), "demo-pipe").join("1_1.json");
    let raw = fs::read_to_string(&one).expect("1_1.json");
    let v: serde_json::Value = serde_json::from_str(&raw).expect("parse");
    assert_eq!(v["code"], 0);
    assert!(v["stdout"].as_str().is_some());
    assert!(v["stderr"].as_str().is_some());
    assert_eq!(v["output"], json!("out1"));
}

#[test]
fn pipelines_shorthand_pipeline_and_prompt_matches_run_output() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "composer-2-fast", "cursor-agent");
    write_pipeline(
        &data_dir,
        "demo-pipe",
        r#"{"steps":[{"id":1,"title":"stepone","prompt":"doprompt","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(
        &mock,
        "#!/bin/sh\ncat >/dev/null\necho '{\"text\":\"out1\"}'\n",
    )
    .expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["pipelines", "--pipeline", "demo-pipe", "--prompt", "userhi"]);
    cmd.assert().success();

    let one = pipeline_artifact_dir_for(temp.path(), "demo-pipe").join("1_1.json");
    let raw = fs::read_to_string(&one).expect("1_1.json");
    let v: serde_json::Value = serde_json::from_str(&raw).expect("parse");
    assert_eq!(v["code"], 0);
    assert_eq!(v["output"], json!("out1"));
}

#[test]
fn pipelines_shorthand_pipeline_and_file_matches_run_output() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "composer-2-fast", "cursor-agent");
    write_pipeline(
        &data_dir,
        "demo-pipe",
        r#"{"steps":[{"id":1,"title":"stepone","prompt":"doprompt","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(
        &mock,
        "#!/bin/sh\ncat >/dev/null\necho '{\"text\":\"out1\"}'\n",
    )
    .expect("mock");
    chmod_x(&mock);

    let pf = temp.path().join("prompt.txt");
    fs::write(&pf, "userhi").expect("pf");

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["pipelines", "--pipeline", "demo-pipe", "--file"])
        .arg(&pf);
    cmd.assert().success();

    let one = pipeline_artifact_dir_for(temp.path(), "demo-pipe").join("1_1.json");
    let raw = fs::read_to_string(&one).expect("1_1.json");
    let v: serde_json::Value = serde_json::from_str(&raw).expect("parse");
    assert_eq!(v["code"], 0);
    assert_eq!(v["output"], json!("out1"));
}

#[test]
fn pipelines_shorthand_file_flag_before_pipeline_flag_matches_run_output() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "composer-2-fast", "cursor-agent");
    write_pipeline(
        &data_dir,
        "demo-pipe",
        r#"{"steps":[{"id":1,"title":"stepone","prompt":"doprompt","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(
        &mock,
        "#!/bin/sh\ncat >/dev/null\necho '{\"text\":\"out1\"}'\n",
    )
    .expect("mock");
    chmod_x(&mock);

    let pf = temp.path().join("user_prompt.txt");
    fs::write(&pf, "from-file-order-test").expect("pf");

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args([
        "pipelines",
        "--file",
        pf.to_str().expect("utf8"),
        "--pipeline",
        "demo-pipe",
    ]);
    cmd.assert().success();

    let one = pipeline_artifact_dir_for(temp.path(), "demo-pipe").join("1_1.json");
    let raw = fs::read_to_string(&one).expect("1_1.json");
    let v: serde_json::Value = serde_json::from_str(&raw).expect("parse");
    assert_eq!(v["code"], 0);
    assert_eq!(v["output"], json!("out1"));
}

#[test]
fn pipelines_shorthand_pipeline_and_file_reads_file_into_agent_stdin() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let stdin_log = temp.path().join("stdin.log");
    let mock = bin.join("cursor-agent");
    let body = format!(
        "#!/bin/sh\ncat > \"{}\"\necho '{{\"text\":\"x\"}}'\n",
        stdin_log.display()
    );
    fs::write(&mock, body).expect("mock");
    chmod_x(&mock);
    let pf = temp.path().join("unique-prompt-file.txt");
    fs::write(&pf, "unique-file-content-shorthand-abc").expect("pf");

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["pipelines", "--pipeline", "pl", "--file"])
        .arg(&pf);
    cmd.assert().success();

    let s = fs::read_to_string(&stdin_log).expect("stdin");
    assert!(s.contains("unique-file-content-shorthand-abc"));
}

#[test]
fn pipelines_shorthand_pipeline_and_file_with_data_dir_cli_flag_only() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("skill-issues");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "composer-2-fast", "cursor-agent");
    write_pipeline(
        &data_dir,
        "prime-executor",
        r#"{"steps":[{"id":1,"title":"stepone","prompt":"doprompt","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(
        &mock,
        "#!/bin/sh\ncat >/dev/null\necho '{\"text\":\"cli-flag\"}'\n",
    )
    .expect("mock");
    chmod_x(&mock);

    let pf = temp.path().join("title");
    fs::write(&pf, "issue body").expect("title");

    let path_var = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut cmd = cargo_bin_cmd!("prime-agent");
    cmd.current_dir(temp.path())
        .env("PATH", &path_var)
        .env("PRIME_AGENT_NO_TUI", "1")
        .env("XDG_CONFIG_HOME", temp.path().join("config"))
        .args([
            "--data-dir",
            data_dir.to_str().expect("utf8"),
            "--skills-dir",
            skills_dir.to_str().expect("utf8"),
            "pipelines",
            "--pipeline",
            "prime-executor",
            "--file",
            pf.to_str().expect("utf8"),
        ]);
    cmd.assert().success();

    let one = pipeline_artifact_dir_for(temp.path(), "prime-executor").join("1_1.json");
    let raw = fs::read_to_string(&one).expect("1_1.json");
    let v: serde_json::Value = serde_json::from_str(&raw).expect("parse");
    assert_eq!(v["code"], 0);
    assert_eq!(v["output"], json!("cli-flag"));
}

#[test]
fn pipelines_shorthand_relative_title_file_in_cwd() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    write_pipeline(
        &data_dir,
        "demo-pipe",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    fs::write(temp.path().join("title"), "relative-title-content\n").expect("title");
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(
        &mock,
        "#!/bin/sh\ncat >/dev/null\necho '{\"text\":\"rel\"}'\n",
    )
    .expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["pipelines", "--pipeline", "demo-pipe", "--file", "title"]);
    cmd.assert().success();

    let one = pipeline_artifact_dir_for(temp.path(), "demo-pipe").join("1_1.json");
    let raw = fs::read_to_string(&one).expect("1_1.json");
    let v: serde_json::Value = serde_json::from_str(&raw).expect("parse");
    assert_eq!(v["code"], 0);
    assert_eq!(v["output"], json!("rel"));
}

#[test]
fn pipelines_shorthand_pipeline_without_prompt_or_file_errors() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["pipelines", "--pipeline", "pl"]);
    cmd.assert().failure().stderr(contains(
        "with --pipeline, provide exactly one of --prompt or --file",
    ));
}

#[test]
fn pipelines_shorthand_pipeline_both_prompt_and_file_errors() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let f = temp.path().join("f.txt");
    fs::write(&f, "a").expect("f");

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args([
        "pipelines",
        "--pipeline",
        "pl",
        "--prompt",
        "x",
        "--file",
        f.to_str().expect("utf8"),
    ]);
    cmd.assert()
        .failure()
        .stderr(contains("use only one of --prompt or --file"));
}

#[test]
fn pipelines_file_without_pipeline_non_tty_lists_does_not_run() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_pipeline(
        &data_dir,
        "listed",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(&mock, "#!/bin/sh\necho '{\"text\":\"x\"}'\n").expect("mock");
    chmod_x(&mock);

    let pf = temp.path().join("ignored.txt");
    fs::write(&pf, "should-not-run-pipeline").expect("pf");

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["pipelines", "--file"]).arg(&pf);
    let out = cmd.assert().success().get_output().stdout.clone();
    assert_eq!(
        stdout_after_version_line(&out),
        "listed\n",
        "--file without --pipeline must not invoke pipeline run; stdout should list names only"
    );
    assert!(
        !temp.path().join(".prime-agent/pipelines").exists(),
        "no run artifacts when only --file is passed without --pipeline"
    );
}

#[test]
fn pipelines_run_rejects_combined_pipeline_flag() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(
        &mock,
        "#!/bin/sh\ncat >/dev/null\necho '{\"text\":\"x\"}'\n",
    )
    .expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args([
        "pipelines",
        "--pipeline",
        "pl",
        "run",
        "pl",
        "--prompt",
        "x",
    ]);
    cmd.assert()
        .failure()
        .stderr(contains("do not combine `pipelines run` with --pipeline"));
}

#[test]
fn pipelines_run_parallel_skills_two_outputs() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(skills_dir.join("alpha")).expect("a");
    fs::create_dir_all(skills_dir.join("beta")).expect("b");
    fs::write(skills_dir.join("alpha/SKILL.md"), "A\n").expect("a");
    fs::write(skills_dir.join("beta/SKILL.md"), "B\n").expect("b");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"t","prompt":"p","skills":["beta","alpha"]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(
        &mock,
        "#!/bin/sh\ncat >/dev/null\necho '{\"text\":\"parallel\"}'\n",
    )
    .expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["pipelines", "run", "pl", "--prompt", "u"]);
    cmd.assert().success();

    let out_dir = pipeline_artifact_dir_for(temp.path(), "pl");
    for (task, sk) in [("1_1", "alpha"), ("1_2", "beta")] {
        let p = out_dir.join(format!("{task}.json"));
        let raw = fs::read_to_string(&p).expect("task json");
        let v: serde_json::Value = serde_json::from_str(&raw).expect("parse");
        assert_eq!(v["code"], 0);
        assert_eq!(v["output"], json!("parallel"));
        assert!(
            v["prompt"].as_str().is_some_and(|s| s.contains(sk)),
            "expected skill {sk} in prompt: {:?}",
            v["prompt"]
        );
    }
}

#[test]
fn pipelines_run_second_stage_receives_prior_stage_files_in_prompt() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"s1","prompt":"p1","skills":[]},{"id":2,"title":"s2","prompt":"p2","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let stdin_log = temp.path().join("stdin.log");
    let mock = bin.join("cursor-agent");
    let body = format!(
        "#!/bin/sh\ncat > \"{}\"\necho '{{\"text\":\"ok\"}}'\n",
        stdin_log.display()
    );
    fs::write(&mock, body).expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["pipelines", "run", "pl", "--prompt", "u"]);
    cmd.assert().success();

    let stage2 = fs::read_to_string(&stdin_log).expect("stdin");
    assert!(
        stage2.contains("### Task file 1_1.json") || stage2.contains("\"code\":0"),
        "expected prior stage content in stage-2 prompt: {stage2}"
    );
}

#[test]
fn pipelines_run_resume_skips_completed_stage() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"s1","prompt":"p1","skills":[]},{"id":2,"title":"s2","prompt":"p2","skills":[]}]}"#,
    );
    let out_dir = temp.path().join(".prime-agent/pipelines/quiet-harbor");
    fs::create_dir_all(&out_dir).expect("out");
    fs::write(
        out_dir.join("meta.json"),
        r#"{"run_name":"quiet-harbor","pipeline":"pl","model":"m","clirunner":"cursor-agent"}
"#,
    )
    .expect("meta");
    fs::write(
        out_dir.join("1_1.json"),
        r#"{"command":"c","user_prompt":"u","skill_prompt":"","pipeline_prompt":"p1","prompt":"x","stdout":"so","stderr":"","code":0,"output":"done"}
"#,
    )
    .expect("1_1.json");

    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let argv_log = temp.path().join("argv.log");
    let mock = bin.join("cursor-agent");
    let body = format!(
        "#!/bin/sh\necho \"$@\" >> \"{}\"\ncat >/dev/null\necho '{{\"text\":\"two\"}}'\n",
        argv_log.display()
    );
    fs::write(&mock, body).expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["pipelines", "run", "pl", "--prompt", "u"]);
    cmd.assert().success();

    let logged = fs::read_to_string(&argv_log).expect("argv log");
    assert_eq!(
        logged.lines().filter(|l| !l.is_empty()).count(),
        1,
        "expected only stage 2 to invoke cursor-agent: {logged}"
    );
}

#[test]
fn pipelines_run_reruns_completed_stage_when_user_prompt_changes() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"s1","prompt":"p1","skills":[]}]}"#,
    );
    let out_dir = temp.path().join(".prime-agent/pipelines/calm-meadow");
    fs::create_dir_all(&out_dir).expect("out");
    fs::write(
        out_dir.join("meta.json"),
        r#"{"run_name":"calm-meadow","pipeline":"pl","model":"m","clirunner":"cursor-agent"}
"#,
    )
    .expect("meta");
    fs::write(
        out_dir.join("1_1.json"),
        r#"{"command":"c","user_prompt":"old-prompt","skill_prompt":"","pipeline_prompt":"p1","prompt":"x","stdout":"so","stderr":"","code":0,"output":"done"}
"#,
    )
    .expect("1_1.json");

    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let argv_log = temp.path().join("argv.log");
    let mock = bin.join("cursor-agent");
    let body = format!(
        "#!/bin/sh\necho \"$@\" >> \"{}\"\ncat >/dev/null\necho '{{\"text\":\"new\"}}'\n",
        argv_log.display()
    );
    fs::write(&mock, body).expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["pipelines", "run", "pl", "--prompt", "new-prompt"]);
    cmd.assert().success();

    let logged = fs::read_to_string(&argv_log).expect("argv log");
    assert_eq!(
        logged.lines().filter(|l| !l.is_empty()).count(),
        1,
        "expected stage 1 to run again when user prompt differs from task json: {logged}"
    );
}

#[test]
fn pipelines_run_prints_kebab_run_name_first_line() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(
        &mock,
        "#!/bin/sh\ncat >/dev/null\necho '{\"text\":\"x\"}'\n",
    )
    .expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["pipelines", "run", "pl", "--prompt", "u"]);
    let out = cmd.assert().success().get_output().stdout.clone();
    let text = String::from_utf8_lossy(&out);
    let first = text
        .lines()
        .find(|l| !l.contains("prime-agent("))
        .expect("line");
    let parts: Vec<&str> = first.split_whitespace().collect();
    assert!(
        parts.len() >= 3 && parts[0] == "pipeline" && parts[1] == "pl",
        "expected `pipeline <kebab> <run name …>`, got: {first:?}"
    );
    let run_name = parts[2];
    assert!(
        run_name.contains('-')
            && run_name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
        "expected lowercase kebab run name, got: {run_name:?}"
    );
}

#[test]
fn pipelines_run_passes_model_and_force_to_subprocess() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config_yolo(&temp, "composer-2-fast", "cursor-agent", true);
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let argv_log = temp.path().join("argv.log");
    let mock = bin.join("cursor-agent");
    let body = format!(
        "#!/bin/sh\necho \"$@\" >> \"{}\"\ncat >/dev/null\necho '{{\"text\":\"x\"}}'\n",
        argv_log.display()
    );
    fs::write(&mock, body).expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["pipelines", "run", "pl", "--prompt", "u"]);
    cmd.assert().success();

    let logged = fs::read_to_string(&argv_log).expect("log");
    assert!(logged.contains("--model"));
    assert!(logged.contains("composer-2-fast"));
    assert!(logged.contains("--force"));
    assert!(!logged.contains("--trust"));
    assert!(!logged.contains("--yolo"));
}

#[test]
fn pipelines_run_omits_force_when_yolo_false() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config_yolo(&temp, "m", "cursor-agent", false);
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let argv_log = temp.path().join("argv.log");
    let mock = bin.join("cursor-agent");
    let body = format!(
        "#!/bin/sh\necho \"$@\" >> \"{}\"\ncat >/dev/null\necho '{{\"text\":\"x\"}}'\n",
        argv_log.display()
    );
    fs::write(&mock, body).expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["pipelines", "run", "pl", "--prompt", "u"]);
    cmd.assert().success();

    let logged = fs::read_to_string(&argv_log).expect("log");
    assert!(!logged.contains("--force"));
}

#[test]
fn pipelines_run_includes_force_when_yolo_key_omitted() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    let d = temp.path().join(".prime-agent");
    fs::create_dir_all(&d).expect("dot dir");
    fs::write(
        d.join("config.json"),
        "{\"model\":\"m\",\"clirunner\":\"cursor-agent\"}\n",
    )
    .expect("dot config");
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let argv_log = temp.path().join("argv.log");
    let mock = bin.join("cursor-agent");
    let body = format!(
        "#!/bin/sh\necho \"$@\" >> \"{}\"\ncat >/dev/null\necho '{{\"text\":\"x\"}}'\n",
        argv_log.display()
    );
    fs::write(&mock, body).expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["pipelines", "run", "pl", "--prompt", "u"]);
    cmd.assert().success();

    let logged = fs::read_to_string(&argv_log).expect("log");
    assert!(logged.contains("--force"));
}

#[test]
fn pipelines_run_reads_prompt_from_file() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let stdin_log = temp.path().join("stdin.log");
    let mock = bin.join("cursor-agent");
    let body = format!(
        "#!/bin/sh\ncat > \"{}\"\necho '{{\"text\":\"x\"}}'\n",
        stdin_log.display()
    );
    fs::write(&mock, body).expect("mock");
    chmod_x(&mock);
    let pf = temp.path().join("user.txt");
    fs::write(&pf, "unique-file-content-xyz").expect("pf");

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["pipelines", "run", "pl", "--file"]).arg(&pf);
    cmd.assert().success();

    let s = fs::read_to_string(&stdin_log).expect("stdin");
    assert!(s.contains("unique-file-content-xyz"));
}

#[test]
fn pipelines_run_stage_json_includes_stdout_stderr_and_error_on_failure() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(&mock, "#!/bin/sh\necho boom >&2\nexit 7\n").expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["pipelines", "run", "pl", "--prompt", "u"]);
    cmd.assert().failure();

    let raw = fs::read_to_string(pipeline_artifact_dir_for(temp.path(), "pl").join("1_1.json"))
        .expect("1_1.json");
    let v: serde_json::Value = serde_json::from_str(&raw).expect("parse");
    assert!(v.get("error").is_some());
    assert_eq!(v["code"], 7);
    assert!(v["stdout"].as_str().is_some());
    assert!(v["stderr"].as_str().is_some());
}

#[test]
fn pipelines_run_stage1_failure_does_not_write_stage2_task_json() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"s1","prompt":"p1","skills":[]},{"id":2,"title":"s2","prompt":"p2","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(&mock, "#!/bin/sh\ncat >/dev/null\necho boom >&2\nexit 7\n").expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["pipelines", "run", "pl", "--prompt", "u"]);
    cmd.assert().failure();

    let d = pipeline_artifact_dir_for(temp.path(), "pl");
    assert!(
        d.join("1_1.json").is_file(),
        "stage 1 task should be written"
    );
    assert!(
        !d.join("2_1.json").exists(),
        "stage 2 must not run after stage 1 failure"
    );
}

/// Braille spinner frames (must match [`prime_agent::pipeline_progress`]).
const SPINNER_CHARS: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

fn strip_ansi_progress(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\r' {
            continue;
        }
        if c == '\x1b' && it.peek() == Some(&'[') {
            it.next();
            for ch in it.by_ref() {
                if ch == 'm' {
                    break;
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

fn normalize_pipeline_header_line(line: &str) -> String {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 3 && parts[0] == "pipeline" {
        return format!("pipeline {} <run_name>", parts[1]);
    }
    line.to_string()
}

fn normalize_running_stdout_counts(line: &str) -> String {
    if !line.contains("* running ") {
        return line.to_string();
    }
    let Some(pos_open) = line.rfind(" (") else {
        return line.to_string();
    };
    let after_open = &line[pos_open + 2..];
    let Some(without_close) = after_open.strip_suffix(')') else {
        return line.to_string();
    };
    let mut parts = without_close.splitn(2, ',');
    let (Some(a), Some(b)) = (parts.next(), parts.next()) else {
        return line.to_string();
    };
    let a = a.trim();
    let b = b.trim();
    if !(a.chars().all(|c| c.is_ascii_digit()) && b.chars().all(|c| c.is_ascii_digit())) {
        return line.to_string();
    }
    format!("{} (<counts>)", &line[..pos_open])
}

fn normalize_spinner_and_secs_line(line: &str) -> String {
    let mut s = line.to_string();
    for sp in SPINNER_CHARS {
        s = s.replace(sp, "<spinner>");
    }
    if let Some(sp) = s.rfind(' ') {
        let tail = &s[sp + 1..];
        if let Some(num) = tail.strip_suffix('s')
            && !num.is_empty()
            && num.chars().all(|c| c.is_ascii_digit())
        {
            return format!("{} <secs>s", &s[..sp]);
        }
    }
    s
}

fn normalize_pipeline_run_stdout(raw: &str) -> String {
    let raw = strip_ansi_progress(raw);
    let mut out = String::new();
    let mut first = true;
    for line in raw.lines() {
        if !first {
            out.push('\n');
        }
        first = false;
        let line = if line.starts_with("prime-agent(") {
            "prime-agent(<version>)".to_string()
        } else if line.starts_with("pipeline ") {
            normalize_pipeline_header_line(line)
        } else {
            let line = normalize_running_stdout_counts(line);
            normalize_spinner_and_secs_line(&line)
        };
        out.push_str(&line);
    }
    out.push('\n');
    out
}

const PIPELINE_RUN_STDOUT_GOLDEN: &str = r#"prime-agent(<version>)
pipeline demo-pipe <run_name>
step 1 stepone
  * running (no skill) (<counts>)
Step 0 / 1 Pipeline 0 / 1 <spinner> <secs>s
step 1 skill (no skill) succeeded, 1 / 1 completed
Step 1 / 1 Pipeline 1 / 1 <spinner> <secs>s
"#;

#[test]
fn pipelines_run_stdout_matches_golden_after_normalization() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config_yolo(&temp, "m", "cursor-agent", true);
    write_pipeline(
        &data_dir,
        "demo-pipe",
        r#"{"steps":[{"id":1,"title":"stepone","prompt":"doprompt","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(
        &mock,
        "#!/bin/sh\ncat >/dev/null\necho '{\"text\":\"out1\"}'\n",
    )
    .expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["pipelines", "run", "demo-pipe", "--prompt", "userhi"]);
    let out = cmd.assert().success().get_output().stdout.clone();
    let got = normalize_pipeline_run_stdout(&String::from_utf8_lossy(&out));
    assert_eq!(got, PIPELINE_RUN_STDOUT_GOLDEN);
}

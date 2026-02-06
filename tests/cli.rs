use assert_cmd::cargo::cargo_bin_cmd;
use assert_cmd::Command;
use predicates::str::contains;
use std::fs;
use std::path::{Path, PathBuf};
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
    let config = format!(
        "{{\n  \"skills-dir\": \"{}\"\n}}\n",
        skills_dir.display()
    );
    fs::write(&config_path, config).expect("write config");
    config_path
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
    assert!(updated.contains("<!-- prime-agent(Start alpha) -->"));
}

#[test]
fn sync_adds_new_skill_to_agents() {
    let temp = TempDir::new().expect("temp dir");
    let skills_dir = temp.path().join("skills");
    write_config(&temp, &skills_dir);
    fs::create_dir_all(&skills_dir).expect("skills dir");
    fs::create_dir_all(skills_dir.join("alpha")).expect("alpha dir");
    fs::write(skills_dir.join("alpha/SKILL.md"), "From skill\n").expect("skill");

    let mut cmd = cmd_with_skills_dir(&temp, &skills_dir);
    cmd.arg("sync");
    cmd.assert().success();

    let agents = fs::read_to_string(default_agents_path(&temp)).expect("agents");
    assert!(agents.contains("<!-- prime-agent(Start alpha) -->"));
    assert!(agents.contains("From skill"));
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
fn env_override_sets_skills_dir() {
    let temp = TempDir::new().expect("temp dir");
    let skills_dir = temp.path().join("custom_skills");
    write_config(&temp, &skills_dir);
    let source = temp.path().join("source.md");
    fs::write(&source, "Env content\n").expect("source");

    let mut cmd = cargo_bin_cmd!("prime-agent");
    cmd.current_dir(temp.path())
        .env("PRIME_AGENT_SKILLS_DIR", &skills_dir)
        .env("XDG_CONFIG_HOME", temp.path().join("config"))
        .arg("set")
        .arg("alpha")
        .arg(&source);
    cmd.assert().success();

    let skill = fs::read_to_string(skills_dir.join("alpha/SKILL.md")).expect("skill");
    assert!(skill.contains("Env content"));
}

#[test]
fn config_sets_skills_dir_when_no_flag() {
    let temp = TempDir::new().expect("temp dir");
    let skills_dir = temp.path().join("skills");
    let source = temp.path().join("source.md");
    fs::write(&source, "Config content\n").expect("source");
    let config_path = write_config(&temp, &skills_dir);

    let mut cmd = cargo_bin_cmd!("prime-agent");
    cmd.current_dir(temp.path())
        .env("XDG_CONFIG_HOME", config_path.parent().unwrap().parent().unwrap())
        .arg("set")
        .arg("alpha")
        .arg(&source);
    cmd.assert().success();

    let skill = fs::read_to_string(skills_dir.join("alpha/SKILL.md")).expect("skill");
    assert!(skill.contains("Config content"));
}

#[test]
fn missing_skills_dir_errors_without_flag_or_config() {
    let temp = TempDir::new().expect("temp dir");
    let source = temp.path().join("source.md");
    fs::write(&source, "Config content\n").expect("source");

    let mut cmd = cargo_bin_cmd!("prime-agent");
    cmd.current_dir(temp.path())
        .env("XDG_CONFIG_HOME", temp.path().join("missing-config"))
        .arg("set")
        .arg("alpha")
        .arg(&source);
    cmd.assert().failure();
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
    cmd.assert()
        .success()
        .stdout(contains("alpha\n"))
        .stdout(contains("beta\n"));
}

mod common;

use assert_cmd::cargo::cargo_bin_cmd;
use common::{cmd_with_skills_dir, write_config};
use std::fs;
use tempfile::TempDir;

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

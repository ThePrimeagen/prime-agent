mod common;

use common::{cmd_with_skills_dir, default_agents_path, write_config};
use std::fs;
use tempfile::TempDir;

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

mod common;

use common::{cmd_with_skills_dir, default_agents_path, write_config};
use predicates::str::contains;
use std::fs;
use tempfile::TempDir;

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

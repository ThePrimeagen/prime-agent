mod common;

use common::{cmd_with_skills_dir, default_agents_path, write_config};
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains as contains_text;
use std::fs;
use tempfile::TempDir;

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
        .stdout(contains_text("alpha (out of sync: conflict)\n"));
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
        .stdout(contains_text("alpha (out of sync: remote)\n"));
}

mod common;

use common::{
    cmd_with_skills_dir, default_agents_path, write_config,
};
use predicates::str::contains;
use std::fs;
use tempfile::TempDir;

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

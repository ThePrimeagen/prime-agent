mod common;

use common::{
    cmd_with_skills_dir, default_agents_path, git_output, run_git, write_config,
};
use std::fs;
use tempfile::TempDir;

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

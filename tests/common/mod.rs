//! Shared helpers for CLI integration tests.
#![allow(dead_code)]
// Each `tests/*.rs` crate compiles this module separately; not every helper is used in every crate.

use assert_cmd::Command;
use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::json;
use std::fs;
use uuid::Uuid;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::time::SystemTime;
use tempfile::TempDir;

pub fn cmd_with_skills_dir(temp: &TempDir, skills_dir: &Path) -> Command {
    let mut cmd = cargo_bin_cmd!("prime-agent");
    cmd.current_dir(temp.path())
        .env("XDG_CONFIG_HOME", temp.path().join("config"))
        .arg("--skills-dir")
        .arg(skills_dir);
    cmd
}

pub fn default_agents_path(temp: &TempDir) -> PathBuf {
    temp.path().join("AGENTS.md")
}

pub fn write_config(temp: &TempDir, skills_dir: &Path) -> PathBuf {
    let config_dir = temp.path().join("config/prime-agent");
    fs::create_dir_all(&config_dir).expect("config dir");
    let config_path = config_dir.join("config");
    let config = format!("{{\n  \"skills-dir\": \"{}\"\n}}\n", skills_dir.display());
    fs::write(&config_path, config).expect("write config");
    config_path
}

pub fn run_git(dir: &Path, args: &[&str]) {
    let status = ProcessCommand::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .status()
        .expect("git command");
    assert!(status.success());
}

pub fn git_output(dir: &Path, args: &[&str]) -> String {
    let output = ProcessCommand::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("git output");
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout).to_string()
}

pub fn write_dot_prime_agent_config(temp: &TempDir, model: &str, clirunner: &str) {
    write_dot_prime_agent_config_yolo(temp, model, clirunner, false);
}

pub fn write_dot_prime_agent_config_yolo(temp: &TempDir, model: &str, clirunner: &str, yolo: bool) {
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

pub fn chmod_x(path: &Path) {
    #[cfg(unix)]
    {
        let mut perms = fs::metadata(path).expect("meta").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("chmod");
    }
}

pub const SKILL_ID_FILE: &str = ".prime-agent-skill-id";

/// Create `skills/<name>/` with `SKILL.md` and a stable UUID id file (matches production layout).
pub fn write_skill_with_id(skills_dir: &Path, name: &str, id: &Uuid, skill_md: &str) {
    let dir = skills_dir.join(name);
    fs::create_dir_all(&dir).expect("skill dir");
    fs::write(dir.join("SKILL.md"), skill_md).expect("SKILL.md");
    fs::write(dir.join(SKILL_ID_FILE), format!("{id}\n")).expect("skill id file");
}

pub fn write_pipeline(data_dir: &Path, name: &str, steps: &str) {
    let dir = data_dir.join("pipelines").join(name);
    fs::create_dir_all(&dir).expect("pipeline dir");
    fs::write(dir.join("pipeline.json"), steps).expect("pipeline.json");
}

/// Latest mtime among `meta.json` and task `*.json` files in a run directory (avoids picking an
/// older run when `meta.json` and task JSON are written in different order).
fn run_dir_recency_mtime(slug_dir: &Path) -> SystemTime {
    let mut best = SystemTime::UNIX_EPOCH;
    let Ok(rd) = fs::read_dir(slug_dir) else {
        return best;
    };
    for e in rd.flatten() {
        let path = e.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_some_and(|x| x == "json")
            && let Ok(m) = fs::metadata(&path).and_then(|m| m.modified())
            && m > best
        {
            best = m;
        }
    }
    best
}

/// After a single `run` in an isolated temp cwd, expects exactly one slug directory under
/// `.prime-agent/pipelines/`. Panics with directory listing if that is not true.
pub fn lone_pipeline_run_dir(cwd: &Path) -> PathBuf {
    let root = cwd.join(".prime-agent/pipelines");
    let mut dirs: Vec<PathBuf> = fs::read_dir(&root)
        .unwrap_or_else(|e| panic!("read_dir {:?}: {}", root, e))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    assert_eq!(
        dirs.len(),
        1,
        "expected exactly one pipeline run directory under {:?}, got {:?}",
        root,
        dirs
    );
    dirs.pop().expect("one dir")
}

/// Run artifacts live under `cwd/.prime-agent/pipelines/<adj-noun-slug>/`; find by `meta.json` `pipeline` field.
/// When several runs exist for the same pipeline, returns the directory with the **newest** task/meta activity.
pub fn pipeline_artifact_dir_for(cwd: &Path, pipeline_name: &str) -> PathBuf {
    let root = cwd.join(".prime-agent/pipelines");
    let rd = fs::read_dir(&root).unwrap_or_else(|e| panic!("read_dir {:?}: {}", root, e));
    let mut best: Option<(PathBuf, SystemTime)> = None;
    for entry in rd {
        let p = entry.expect("entry").path();
        if !p.is_dir() {
            continue;
        }
        let meta_path = p.join("meta.json");
        if !meta_path.is_file() {
            continue;
        }
        let raw = fs::read_to_string(&meta_path).expect("meta");
        let v: serde_json::Value = serde_json::from_str(&raw).expect("parse meta");
        if v.get("pipeline").and_then(|x| x.as_str()) == Some(pipeline_name) {
            let mt = run_dir_recency_mtime(&p);
            let replace = match &best {
                None => true,
                Some((prev_p, prev_t)) => mt > *prev_t || (mt == *prev_t && p > *prev_p),
            };
            if replace {
                best = Some((p, mt));
            }
        }
    }
    best.map(|(p, _)| p)
        .unwrap_or_else(|| panic!("no run dir for pipeline '{pipeline_name}' under {:?}", root))
}

/// Everything after the first two lines of stdout (banner + effective config summary).
pub fn stdout_after_version_line(stdout: &[u8]) -> String {
    let text = String::from_utf8_lossy(stdout);
    let mut start = 0_usize;
    for _ in 0..2 {
        if let Some(pos) = text[start..].find('\n') {
            start += pos + 1;
        } else {
            return String::new();
        }
    }
    text[start..].to_string()
}

/// Base `prime-agent` invocation with `--data-dir`, `--skills-dir`, and PATH to a mock `cursor-agent`.
pub fn pipelines_cmd(temp: &TempDir, data_dir: &Path, skills_dir: &Path, bin_dir: &Path) -> Command {
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

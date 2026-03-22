//! After skill edits go idle, run `cursor-agent` to commit and push the data directory.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use serde_json::Value;

use crate::pipeline_run::run_cursor_agent_streaming;
use crate::sync::{git_is_clean, git_is_repo};

const SUPPORTED_CLIRUNNER: &str = "cursor-agent";

/// Tracks last skill mutation time for idle `cursor-agent` commit.
pub struct SkillActivity {
    pub last_mutation: std::sync::Mutex<Option<Instant>>,
    pub idle_commit_in_flight: AtomicBool,
}

impl SkillActivity {
    #[must_use]
    pub fn new() -> Self {
        Self {
            last_mutation: std::sync::Mutex::new(None),
            idle_commit_in_flight: AtomicBool::new(false),
        }
    }

    pub fn mark_mutation(&self) {
        *self.last_mutation.lock().expect("skill activity lock") = Some(Instant::now());
    }
}

impl Default for SkillActivity {
    fn default() -> Self {
        Self::new()
    }
}

fn idle_commit_secs() -> u64 {
    std::env::var("PRIME_AGENT_IDLE_COMMIT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(60)
        .max(1)
}

fn load_cursor_config_for_idle() -> Option<(String, String)> {
    let path = Path::new(".prime-agent").join("config.json");
    let raw = std::fs::read_to_string(&path).ok()?;
    let v: Value = serde_json::from_str(&raw).ok()?;
    let model = v.get("model").and_then(|x| x.as_str())?.to_string();
    let clirunner = v.get("clirunner").and_then(|x| x.as_str())?.to_string();
    if model.is_empty() || clirunner.is_empty() {
        return None;
    }
    Some((model, clirunner))
}

fn idle_commit_prompt() -> &'static str {
    "You are working in the git repository at the workspace path (--workspace). \
If there are uncommitted changes, run `git add -A`, then `git commit` with a short message describing the updates, \
then `git push origin` (or `git push -u origin HEAD` if no upstream branch is set). \
If there is nothing to commit or the working tree is clean, respond briefly and exit. \
Do not ask for confirmation."
}

fn run_idle_commit_cursor(data_dir: &Path) -> Result<()> {
    let Some((model, clirunner)) = load_cursor_config_for_idle() else {
        return Ok(());
    };
    if clirunner != SUPPORTED_CLIRUNNER {
        return Ok(());
    }

    let (_stdout, stderr, result) =
        run_cursor_agent_streaming(&clirunner, &model, data_dir, idle_commit_prompt(), None);
    result
        .map(|_| ())
        .map_err(|e| anyhow::anyhow!("cursor-agent: {e}; stderr={}", stderr.trim()))
}

/// Spawn a background task that periodically commits and pushes `data_dir` when skill edits are idle.
pub fn spawn_idle_commit_task(data_dir: PathBuf, skill_activity: Arc<SkillActivity>) {
    tokio::spawn(async move {
        let idle_secs = idle_commit_secs();
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;

            let last = {
                let g = skill_activity.last_mutation.lock().expect("skill activity lock");
                *g
            };
            let Some(t) = last else {
                continue;
            };
            if t.elapsed() < Duration::from_secs(idle_secs) {
                continue;
            }

            match git_is_repo(&data_dir) {
                Ok(false) | Err(_) => continue,
                Ok(true) => {}
            }
            match git_is_clean(&data_dir) {
                Ok(false) => {}
                Ok(true) | Err(_) => continue,
            }

            if skill_activity
                .idle_commit_in_flight
                .swap(true, Ordering::SeqCst)
            {
                continue;
            }

            let dd = data_dir.clone();
            let sa = Arc::clone(&skill_activity);
            let join = tokio::task::spawn_blocking(move || run_idle_commit_cursor(&dd));

            let outcome = join.await;
            skill_activity
                .idle_commit_in_flight
                .store(false, Ordering::SeqCst);

            match outcome {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    eprintln!("idle commit failed: {e:#}");
                    sa.mark_mutation();
                }
                Err(e) => {
                    eprintln!("idle commit task join error: {e}");
                    sa.mark_mutation();
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    #[test]
    fn git_helpers_in_temp_repo() {
        let dir = TempDir::new().expect("tempdir");
        let root = dir.path();
        let status = Command::new("git")
            .args(["init", root.to_str().expect("utf8")])
            .status()
            .expect("git init");
        assert!(status.success());
        assert!(git_is_repo(root).expect("ok"));
        assert!(git_is_clean(root).expect("ok"));
    }
}

//! Sync status between skills on disk and AGENTS.md sections (`local` command).

use crate::agents_md::AgentsDoc;
use crate::skills_store::SkillsStore;
use anyhow::{Context, Result, bail};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::process::Command;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncStatus {
    InSync,
    Local,
    Remote,
    Conflict,
}

pub fn compute_sync_status(
    skills_store: &SkillsStore,
    agents_doc: Option<&AgentsDoc>,
) -> Result<BTreeMap<String, SyncStatus>> {
    if agents_doc.is_none() || agents_doc.is_some_and(|doc| doc.section_names().is_empty()) {
        return Ok(BTreeMap::new());
    }
    let mut skills_map = HashMap::new();
    for name in skills_store.list_skill_names()? {
        let content = skills_store.load_skill(&name)?;
        skills_map.insert(name, normalize_content(&content));
    }
    let mut agents_map = HashMap::new();
    if let Some(doc) = agents_doc {
        for name in doc.section_names() {
            if let Some(section) = doc.get_section(&name) {
                agents_map.insert(name, normalize_content(&section.content_string()));
            }
        }
    }

    let mut names = BTreeSet::new();
    names.extend(skills_map.keys().cloned());
    names.extend(agents_map.keys().cloned());

    let mut statuses = BTreeMap::new();
    for name in names {
        match (skills_map.get(&name), agents_map.get(&name)) {
            (Some(local), Some(remote)) => {
                if local == remote {
                    statuses.insert(name, SyncStatus::InSync);
                } else {
                    statuses.insert(name, SyncStatus::Conflict);
                }
            }
            (Some(_), None) => {
                statuses.insert(name, SyncStatus::Local);
            }
            (None, Some(_)) => {
                statuses.insert(name, SyncStatus::Remote);
            }
            (None, None) => {}
        }
    }
    Ok(statuses)
}

fn normalize_content(content: &str) -> String {
    content
        .replace("\r\n", "\n")
        .trim_end_matches('\n')
        .to_string()
}

pub(crate) fn git_is_repo(root: &Path) -> Result<bool> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("rev-parse")
        .arg("--is-inside-work-tree")
        .output()
        .context("failed to run git rev-parse")?;
    Ok(output.status.success())
}

pub(crate) fn git_is_clean(root: &Path) -> Result<bool> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("status")
        .arg("--porcelain")
        .output()
        .context("failed to run git status")?;
    if !output.status.success() {
        bail!("git status failed");
    }
    Ok(output.stdout.is_empty())
}

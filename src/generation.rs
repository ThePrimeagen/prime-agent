//! Per-skill / per-pipeline generation counters for live-reload client gating.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::pipeline_store::PipelineStore;
use crate::skills_store::SkillsStore;

#[derive(Clone, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
pub struct GenerationSnapshot {
    #[serde(default)]
    pub skills: HashMap<String, u64>,
    #[serde(default)]
    pub pipelines: HashMap<String, u64>,
    #[serde(default)]
    pub list_epoch: u64,
}

#[derive(Debug)]
pub struct GenerationRegistry {
    skills: HashMap<String, u64>,
    pipelines: HashMap<String, u64>,
    list_epoch: u64,
    skill_hash: HashMap<String, u64>,
    pipeline_hash: HashMap<String, u64>,
}

fn hash_bytes(data: &[u8]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    data.hash(&mut h);
    h.finish()
}

impl GenerationRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
            pipelines: HashMap::new(),
            list_epoch: 0,
            skill_hash: HashMap::new(),
            pipeline_hash: HashMap::new(),
        }
    }

    /// Initialize generations and content hashes from disk (call once at startup).
    pub fn bootstrap_from_disk(skills: &SkillsStore, pipelines: &PipelineStore) -> Result<Self> {
        let mut s = Self::new();
        for name in skills.list_skill_names()? {
            let content = skills.load_skill(&name)?;
            let h = hash_bytes(content.as_bytes());
            s.skills.insert(name.clone(), 1);
            s.skill_hash.insert(name, h);
        }
        for name in pipelines.list_pipeline_names()? {
            let raw = std::fs::read_to_string(pipelines.pipeline_json_path(&name))?;
            let h = hash_bytes(raw.as_bytes());
            s.pipelines.insert(name.clone(), 1);
            s.pipeline_hash.insert(name, h);
        }
        if !s.skills.is_empty() || !s.pipelines.is_empty() {
            s.list_epoch = 1;
        }
        Ok(s)
    }

    #[must_use]
    pub fn snapshot(&self) -> GenerationSnapshot {
        GenerationSnapshot {
            skills: self.skills.clone(),
            pipelines: self.pipelines.clone(),
            list_epoch: self.list_epoch,
        }
    }

    pub fn record_skill_write(&mut self, name: &str, content: &str) {
        *self.skills.entry(name.to_string()).or_insert(0) += 1;
        self.skill_hash
            .insert(name.to_string(), hash_bytes(content.as_bytes()));
    }

    pub fn record_skill_rename(&mut self, old: &str, new: &str, new_content: &str) {
        self.skills.remove(old);
        self.skill_hash.remove(old);
        *self.skills.entry(new.to_string()).or_insert(0) += 1;
        self.skill_hash
            .insert(new.to_string(), hash_bytes(new_content.as_bytes()));
        self.list_epoch += 1;
    }

    pub fn record_skill_delete(&mut self, name: &str) {
        self.skills.remove(name);
        self.skill_hash.remove(name);
        self.list_epoch += 1;
    }

    pub fn record_skill_created(&mut self, name: &str, content: &str) {
        *self.skills.entry(name.to_string()).or_insert(0) += 1;
        self.skill_hash
            .insert(name.to_string(), hash_bytes(content.as_bytes()));
        self.list_epoch += 1;
    }

    pub fn record_pipeline_write_from_path(&mut self, name: &str, path: &Path) -> Result<()> {
        let raw = std::fs::read_to_string(path)?;
        self.record_pipeline_raw(name, &raw);
        Ok(())
    }

    pub fn record_pipeline_raw(&mut self, name: &str, raw: &str) {
        *self.pipelines.entry(name.to_string()).or_insert(0) += 1;
        self.pipeline_hash
            .insert(name.to_string(), hash_bytes(raw.as_bytes()));
    }

    pub fn record_pipeline_created(&mut self, name: &str, raw: &str) {
        *self.pipelines.entry(name.to_string()).or_insert(0) += 1;
        self.pipeline_hash
            .insert(name.to_string(), hash_bytes(raw.as_bytes()));
        self.list_epoch += 1;
    }

    /// Bump pipeline generation only when `pipeline.json` content differs from the last hash.
    pub fn reconcile_pipeline_file_content(&mut self, name: &str, raw: &str) {
        let h = hash_bytes(raw.as_bytes());
        match self.pipeline_hash.get(name) {
            Some(prev) if *prev == h => {}
            _ => {
                *self.pipelines.entry(name.to_string()).or_insert(0) += 1;
                self.pipeline_hash.insert(name.to_string(), h);
            }
        }
    }

    /// Re-read disk; bump generations only when file content differs from last known hash.
    pub fn reconcile_from_disk(&mut self, skills: &SkillsStore, pipelines: &PipelineStore) -> Result<bool> {
        let mut changed = false;
        let disk_skills = skills.list_skill_names()?;
        let disk_pipelines = pipelines.list_pipeline_names()?;

        for name in &disk_skills {
            let content = skills.load_skill(name)?;
            let h = hash_bytes(content.as_bytes());
            match self.skill_hash.get(name) {
                None => {
                    *self.skills.entry(name.clone()).or_insert(0) += 1;
                    self.skill_hash.insert(name.clone(), h);
                    self.list_epoch += 1;
                    changed = true;
                }
                Some(prev) if *prev != h => {
                    *self.skills.entry(name.clone()).or_insert(0) += 1;
                    self.skill_hash.insert(name.clone(), h);
                    changed = true;
                }
                Some(_) => {}
            }
        }

        let skill_set: std::collections::HashSet<_> = disk_skills.iter().cloned().collect();
        let stale_skills: Vec<_> = self
            .skills
            .keys()
            .filter(|k| !skill_set.contains(*k))
            .cloned()
            .collect();
        for name in stale_skills {
            self.skills.remove(&name);
            self.skill_hash.remove(&name);
            self.list_epoch += 1;
            changed = true;
        }

        for name in &disk_pipelines {
            let raw = std::fs::read_to_string(pipelines.pipeline_json_path(name))?;
            let h = hash_bytes(raw.as_bytes());
            match self.pipeline_hash.get(name) {
                None => {
                    *self.pipelines.entry(name.clone()).or_insert(0) += 1;
                    self.pipeline_hash.insert(name.clone(), h);
                    self.list_epoch += 1;
                    changed = true;
                }
                Some(prev) if *prev != h => {
                    *self.pipelines.entry(name.clone()).or_insert(0) += 1;
                    self.pipeline_hash.insert(name.clone(), h);
                    changed = true;
                }
                Some(_) => {}
            }
        }

        let pipe_set: std::collections::HashSet<_> = disk_pipelines.iter().cloned().collect();
        let stale_pipes: Vec<_> = self
            .pipelines
            .keys()
            .filter(|k| !pipe_set.contains(*k))
            .cloned()
            .collect();
        for name in stale_pipes {
            self.pipelines.remove(&name);
            self.pipeline_hash.remove(&name);
            self.list_epoch += 1;
            changed = true;
        }

        Ok(changed)
    }
}

pub fn new_registry_mutex(
    skills: &SkillsStore,
    pipelines: &PipelineStore,
) -> Result<Arc<Mutex<GenerationRegistry>>> {
    Ok(Arc::new(Mutex::new(GenerationRegistry::bootstrap_from_disk(
        skills, pipelines,
    )?)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn reconcile_detects_deleted_skill_file() {
        let data = TempDir::new().unwrap();
        let skills_root = data.path().join("skills");
        fs::create_dir_all(skills_root.join("a")).unwrap();
        fs::write(skills_root.join("a").join("SKILL.md"), "hello").unwrap();
        fs::create_dir_all(data.path().join("pipelines")).unwrap();
        let skills_store = SkillsStore::new(skills_root.clone());
        let pipelines = PipelineStore::new(data.path());
        let mut reg = GenerationRegistry::bootstrap_from_disk(&skills_store, &pipelines).unwrap();
        fs::remove_file(skills_store.skill_path("a")).unwrap();
        assert!(reg.reconcile_from_disk(&skills_store, &pipelines).unwrap());
    }

    #[test]
    fn reconcile_reports_no_change_when_disk_matches_registry() {
        let data = TempDir::new().unwrap();
        let skills_root = data.path().join("skills");
        fs::create_dir_all(skills_root.join("a")).unwrap();
        fs::write(skills_root.join("a").join("SKILL.md"), "hello").unwrap();
        fs::create_dir_all(data.path().join("pipelines")).unwrap();
        let skills_store = SkillsStore::new(skills_root);
        let pipelines = PipelineStore::new(data.path());
        let mut reg = GenerationRegistry::bootstrap_from_disk(&skills_store, &pipelines).unwrap();
        assert!(!reg.reconcile_from_disk(&skills_store, &pipelines).unwrap());
        fs::write(skills_store.skill_path("a"), "hello world").unwrap();
        assert!(reg.reconcile_from_disk(&skills_store, &pipelines).unwrap());
        assert!(!reg.reconcile_from_disk(&skills_store, &pipelines).unwrap());
    }
}

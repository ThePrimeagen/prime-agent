//! Filesystem-backed pipelines under `<data-dir>/pipelines/<name>/pipeline.json`.

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const NAME_RULE_MESSAGE: &str = "name must contain only lowercase letters, digits, and dashes";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PipelineFile {
    pub steps: Vec<PipelineStepRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStepRecord {
    pub id: i64,
    pub title: String,
    pub prompt: String,
    #[serde(default)]
    pub skills: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PipelineMeta {
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct PipelineStepView {
    pub id: i64,
    pub title: String,
    pub prompt: String,
    pub skill_count: i64,
    pub skills: Vec<StepSkillView>,
}

#[derive(Debug, Clone)]
pub struct StepSkillView {
    pub name: String,
}

#[derive(Clone)]
pub struct PipelineStore {
    root: PathBuf,
}

impl PipelineStore {
    #[must_use]
    pub fn new(data_dir: &Path) -> Self {
        Self {
            root: data_dir.join("pipelines"),
        }
    }

    fn pipeline_dir(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    fn pipeline_file(&self, name: &str) -> PathBuf {
        self.pipeline_dir(name).join("pipeline.json")
    }

    /// Path to `pipelines/<name>/pipeline.json` (for hashing / generation tracking).
    #[must_use]
    pub fn pipeline_json_path(&self, name: &str) -> PathBuf {
        self.pipeline_file(name)
    }

    pub fn validate_kebab_name(name: &str) -> Result<()> {
        if !name
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
            || name.is_empty()
        {
            return Err(anyhow!(NAME_RULE_MESSAGE));
        }
        Ok(())
    }

    fn read_file(&self, name: &str) -> Result<PipelineFile> {
        let path = self.pipeline_file(name);
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("read pipeline '{}'", path.display()))?;
        let parsed: PipelineFile = serde_json::from_str(&raw)
            .with_context(|| format!("parse pipeline '{}'", path.display()))?;
        Ok(parsed)
    }

    fn write_file_atomic(&self, name: &str, file: &PipelineFile) -> Result<()> {
        Self::validate_kebab_name(name)?;
        let dir = self.pipeline_dir(name);
        fs::create_dir_all(&dir)
            .with_context(|| format!("create pipeline dir '{}'", dir.display()))?;
        let path = dir.join("pipeline.json");
        let tmp = dir.join("pipeline.json.tmp");
        let serialized = serde_json::to_string_pretty(file).context("serialize pipeline")?;
        fs::write(&tmp, format!("{serialized}\n"))
            .with_context(|| format!("write '{}'", tmp.display()))?;
        fs::rename(&tmp, &path).with_context(|| format!("rename to '{}'", path.display()))?;
        Ok(())
    }

    pub fn list_pipeline_names(&self) -> Result<Vec<String>> {
        if !self.root.exists() {
            return Ok(Vec::new());
        }
        let mut names = Vec::new();
        for entry in
            fs::read_dir(&self.root).with_context(|| format!("read '{}'", self.root.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if !path.join("pipeline.json").exists() {
                continue;
            }
            if let Some(stem) = path.file_name().and_then(|s| s.to_str()) {
                names.push(stem.to_string());
            }
        }
        names.sort();
        Ok(names)
    }

    pub fn create_pipeline(&self, name: &str) -> Result<()> {
        Self::validate_kebab_name(name)?;
        let dir = self.pipeline_dir(name);
        if dir.exists() {
            return Err(anyhow!("pipeline '{name}' already exists"));
        }
        fs::create_dir_all(&dir)
            .with_context(|| format!("create pipeline dir '{}'", dir.display()))?;
        let file = PipelineFile::default();
        self.write_file_atomic(name, &file)?;
        Ok(())
    }

    pub fn rename_pipeline(&self, old: &str, new: &str) -> Result<()> {
        Self::validate_kebab_name(old)?;
        Self::validate_kebab_name(new)?;
        if old == new {
            return Err(anyhow!("name unchanged"));
        }
        let old_d = self.pipeline_dir(old);
        let new_d = self.pipeline_dir(new);
        if !old_d.is_dir() || !self.pipeline_file(old).exists() {
            return Err(anyhow!("pipeline not found"));
        }
        if new_d.exists() {
            return Err(anyhow!("pipeline '{new}' already exists"));
        }
        fs::rename(&old_d, &new_d).with_context(|| {
            format!(
                "rename pipeline dir '{}' -> '{}'",
                old_d.display(),
                new_d.display()
            )
        })?;
        Ok(())
    }

    pub fn get_pipeline_meta(&self, name: &str) -> Result<PipelineMeta> {
        if !self.pipeline_file(name).exists() {
            return Err(anyhow!("pipeline not found"));
        }
        Ok(PipelineMeta {
            name: name.to_string(),
        })
    }

    pub fn list_steps(&self, pipeline_name: &str) -> Result<Vec<PipelineStepView>> {
        let file = self.read_file(pipeline_name)?;
        let mut out = Vec::new();
        for step in file.steps {
            let mut skill_names: Vec<String> = step.skills.clone();
            skill_names.sort();
            let skill_count = i64::try_from(skill_names.len()).unwrap_or(i64::MAX);
            let skills: Vec<StepSkillView> = skill_names
                .into_iter()
                .map(|name| StepSkillView { name })
                .collect();
            out.push(PipelineStepView {
                id: step.id,
                title: step.title,
                prompt: step.prompt,
                skill_count,
                skills,
            });
        }
        Ok(out)
    }

    fn next_step_id(file: &PipelineFile) -> i64 {
        file.steps.iter().map(|s| s.id).max().unwrap_or(0) + 1
    }

    pub fn create_step(&self, pipeline_name: &str, title: &str, prompt: &str) -> Result<i64> {
        let title = title.trim().to_lowercase();
        let prompt = prompt.to_string();
        if title.is_empty() || prompt.is_empty() {
            return Err(anyhow!("title and prompt are required"));
        }
        let mut file = self.read_file(pipeline_name)?;
        let id = Self::next_step_id(&file);
        file.steps.push(PipelineStepRecord {
            id,
            title,
            prompt,
            skills: Vec::new(),
        });
        self.write_file_atomic(pipeline_name, &file)?;
        Ok(id)
    }

    pub fn update_step(
        &self,
        pipeline_name: &str,
        step_id: i64,
        title: &str,
        prompt: &str,
    ) -> Result<()> {
        let title = title.trim().to_lowercase();
        let prompt = prompt.to_string();
        if title.is_empty() || prompt.is_empty() {
            return Err(anyhow!("title and prompt are required"));
        }
        let mut file = self.read_file(pipeline_name)?;
        let Some(step) = file.steps.iter_mut().find(|s| s.id == step_id) else {
            return Err(anyhow!("step not found"));
        };
        step.title = title;
        step.prompt = prompt;
        self.write_file_atomic(pipeline_name, &file)?;
        Ok(())
    }

    pub fn delete_step(&self, pipeline_name: &str, step_id: i64) -> Result<()> {
        let mut file = self.read_file(pipeline_name)?;
        let pos = file
            .steps
            .iter()
            .position(|s| s.id == step_id)
            .ok_or_else(|| anyhow!("step not found"))?;
        file.steps.remove(pos);
        self.write_file_atomic(pipeline_name, &file)?;
        Ok(())
    }

    pub fn reorder_step(
        &self,
        pipeline_name: &str,
        step_id: i64,
        target_step_id: i64,
    ) -> Result<()> {
        let mut file = self.read_file(pipeline_name)?;
        let source_idx = file
            .steps
            .iter()
            .position(|s| s.id == step_id)
            .ok_or_else(|| anyhow!("step not found"))?;
        let target_idx = file
            .steps
            .iter()
            .position(|s| s.id == target_step_id)
            .ok_or_else(|| anyhow!("target step not found"))?;
        if source_idx == target_idx {
            return Ok(());
        }
        let step = file.steps.remove(source_idx);
        let mut insert_at = target_idx;
        if source_idx < target_idx {
            insert_at -= 1;
        }
        file.steps.insert(insert_at, step);
        self.write_file_atomic(pipeline_name, &file)?;
        Ok(())
    }

    pub fn add_step_skill(
        &self,
        pipeline_name: &str,
        step_id: i64,
        skill_name: &str,
        skill_exists: impl FnOnce() -> bool,
    ) -> Result<()> {
        if !skill_exists() {
            return Err(anyhow!("skill not found"));
        }
        let mut file = self.read_file(pipeline_name)?;
        let Some(step) = file.steps.iter_mut().find(|s| s.id == step_id) else {
            return Err(anyhow!("step not found"));
        };
        if step.skills.iter().any(|s| s == skill_name) {
            return Err(anyhow!("skill already attached to step"));
        }
        step.skills.push(skill_name.to_string());
        self.write_file_atomic(pipeline_name, &file)?;
        Ok(())
    }

    pub fn delete_step_skill(
        &self,
        pipeline_name: &str,
        step_id: i64,
        skill_name: &str,
    ) -> Result<()> {
        let mut file = self.read_file(pipeline_name)?;
        let Some(step) = file.steps.iter_mut().find(|s| s.id == step_id) else {
            return Err(anyhow!("step not found"));
        };
        let before = step.skills.len();
        step.skills.retain(|s| s != skill_name);
        if step.skills.len() == before {
            return Err(anyhow!("skill link not found"));
        }
        self.write_file_atomic(pipeline_name, &file)?;
        Ok(())
    }

    /// Replace `old` with `new` in every pipeline step skill list.
    pub fn rename_skill_reference(&self, old: &str, new: &str) -> Result<()> {
        if !self.root.exists() {
            return Ok(());
        }
        for entry in
            fs::read_dir(&self.root).with_context(|| format!("read '{}'", self.root.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if !path.join("pipeline.json").exists() {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .map(str::to_string)
                .unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            let mut file = self.read_file(&name)?;
            let mut changed = false;
            for step in &mut file.steps {
                for s in &mut step.skills {
                    if s == old {
                        *s = new.to_string();
                        changed = true;
                    }
                }
            }
            if changed {
                self.write_file_atomic(&name, &file)?;
            }
        }
        Ok(())
    }

    /// Remove `skill` from all pipeline steps.
    pub fn remove_skill_everywhere(&self, skill: &str) -> Result<()> {
        if !self.root.exists() {
            return Ok(());
        }
        let names = self.list_pipeline_names()?;
        for name in names {
            let mut file = self.read_file(&name)?;
            let mut changed = false;
            for step in &mut file.steps {
                let before = step.skills.len();
                step.skills.retain(|s| s != skill);
                if step.skills.len() != before {
                    changed = true;
                }
            }
            if changed {
                self.write_file_atomic(&name, &file)?;
            }
        }
        Ok(())
    }
}

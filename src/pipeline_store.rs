//! Filesystem-backed pipelines under `<data-dir>/pipelines/<name>/pipeline.json`.

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::skills_store::SkillsStore;

pub const NAME_RULE_MESSAGE: &str = "name must contain only lowercase letters, digits, and dashes";

#[derive(Debug, Clone, Serialize, Default)]
pub struct PipelineFile {
    pub steps: Vec<PipelineStepRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineSkillRef {
    pub id: Uuid,
    pub alias: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PipelineStepRecord {
    pub id: i64,
    pub title: String,
    pub prompt: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<PipelineSkillRef>,
}

/// JSON shape for `pipeline.json` on disk: skills may be objects or skill directory names (strings).
#[derive(Debug, Deserialize)]
struct PipelineFileDe {
    steps: Vec<PipelineStepRecordDe>,
}

#[derive(Debug, Deserialize)]
struct PipelineStepRecordDe {
    id: i64,
    title: String,
    prompt: String,
    #[serde(default)]
    skills: Vec<PipelineSkillRefDe>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum PipelineSkillRefDe {
    Short(String),
    Full(PipelineSkillRef),
}

fn hydrate_pipeline_file(
    de: PipelineFileDe,
    skills: &SkillsStore,
    pipeline_path: &str,
) -> Result<(PipelineFile, bool)> {
    let mut canonicalized = false;
    let mut steps = Vec::new();
    for step in de.steps {
        let mut skills_out = Vec::new();
        for att in step.skills {
            match att {
                PipelineSkillRefDe::Full(r) => skills_out.push(r),
                PipelineSkillRefDe::Short(s) => {
                    let trimmed = s.trim();
                    if trimmed.is_empty() {
                        return Err(anyhow!(
                            "empty skill reference in pipeline {pipeline_path}"
                        ));
                    }
                    let normalized = SkillsStore::normalize_skill_name(trimmed);
                    let id = skills
                        .skill_uuid_for_name(&normalized)
                        .with_context(|| {
                            format!(
                                "resolve skill reference '{normalized}' in pipeline {pipeline_path}"
                            )
                        })?;
                    canonicalized = true;
                    skills_out.push(PipelineSkillRef {
                        id,
                        alias: normalized,
                    });
                }
            }
        }
        steps.push(PipelineStepRecord {
            id: step.id,
            title: step.title,
            prompt: step.prompt,
            skills: skills_out,
        });
    }
    Ok((PipelineFile { steps }, canonicalized))
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
    pub id: Uuid,
    pub alias: String,
    /// Directory name under `skills/` when the attachment resolves; `None` if broken.
    pub resolved_name: Option<String>,
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

    fn read_file(&self, name: &str, skills: &SkillsStore) -> Result<PipelineFile> {
        let path = self.pipeline_file(name);
        let ctx = path.display().to_string();
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("read pipeline '{}'", path.display()))?;
        let de: PipelineFileDe = serde_json::from_str(&raw)
            .with_context(|| format!("parse pipeline '{}'", path.display()))?;
        let (file, from_short) = hydrate_pipeline_file(de, skills, &ctx)?;
        if from_short {
            self.write_file_atomic(name, &file)?;
        }
        Ok(file)
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

    /// `true` if any step references a skill id that does not resolve on disk (or duplicate-id index error).
    pub fn pipeline_is_broken(&self, skills: &SkillsStore, pipeline_name: &str) -> bool {
        self.list_steps(skills, pipeline_name)
            .map(|steps| {
                steps.iter().any(|s| {
                    s.skills
                        .iter()
                        .any(|sk| sk.resolved_name.is_none())
                })
            })
            .unwrap_or(true)
    }

    /// Sorted pipeline names paired with broken flag.
    pub fn list_pipelines_with_health(
        &self,
        skills: &SkillsStore,
    ) -> Result<Vec<(String, bool)>> {
        let mut out = Vec::new();
        for name in self.list_pipeline_names()? {
            let broken = self.pipeline_is_broken(skills, &name);
            out.push((name, broken));
        }
        Ok(out)
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

    fn resolve_and_maybe_rewrite(
        &self,
        skills: &SkillsStore,
        pipeline_name: &str,
        mut file: PipelineFile,
    ) -> Result<PipelineFile> {
        let index = skills.uuid_index()?;
        let mut dirty = false;
        for step in &mut file.steps {
            for att in &mut step.skills {
                if let Some(dir) = index.get(&att.id) {
                    let resolved = dir
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_string();
                    if resolved != att.alias {
                        att.alias = resolved;
                        dirty = true;
                    }
                }
            }
        }
        if dirty {
            self.write_file_atomic(pipeline_name, &file)?;
        }
        Ok(file)
    }

    pub fn list_steps(&self, skills: &SkillsStore, pipeline_name: &str) -> Result<Vec<PipelineStepView>> {
        let file = self.read_file(pipeline_name, skills)?;
        let file = self.resolve_and_maybe_rewrite(skills, pipeline_name, file)?;
        let index = skills.uuid_index()?;
        let mut out = Vec::new();
        for step in file.steps {
            let mut skill_views: Vec<StepSkillView> = Vec::new();
            for att in &step.skills {
                let resolved_name = index.get(&att.id).and_then(|dir| {
                    dir.file_name()
                        .and_then(|s| s.to_str())
                        .map(str::to_string)
                });
                skill_views.push(StepSkillView {
                    id: att.id,
                    alias: att.alias.clone(),
                    resolved_name,
                });
            }
            skill_views.sort_by(|a, b| {
                a.resolved_name
                    .as_ref()
                    .or(Some(&a.alias))
                    .cmp(&b.resolved_name.as_ref().or(Some(&b.alias)))
            });
            let skill_count = i64::try_from(skill_views.len()).unwrap_or(i64::MAX);
            out.push(PipelineStepView {
                id: step.id,
                title: step.title,
                prompt: step.prompt,
                skill_count,
                skills: skill_views,
            });
        }
        Ok(out)
    }

    fn next_step_id(file: &PipelineFile) -> i64 {
        file.steps.iter().map(|s| s.id).max().unwrap_or(0) + 1
    }

    pub fn create_step(
        &self,
        skills: &SkillsStore,
        pipeline_name: &str,
        title: &str,
        prompt: &str,
    ) -> Result<i64> {
        let title = title.trim().to_lowercase();
        let prompt = prompt.to_string();
        if title.is_empty() {
            return Err(anyhow!("title is required"));
        }
        let mut file = self.read_file(pipeline_name, skills)?;
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
        skills: &SkillsStore,
        pipeline_name: &str,
        step_id: i64,
        title: &str,
        prompt: &str,
    ) -> Result<()> {
        let title = title.trim().to_lowercase();
        let prompt = prompt.to_string();
        if title.is_empty() {
            return Err(anyhow!("title is required"));
        }
        let mut file = self.read_file(pipeline_name, skills)?;
        let Some(step) = file.steps.iter_mut().find(|s| s.id == step_id) else {
            return Err(anyhow!("step not found"));
        };
        step.title = title;
        step.prompt = prompt;
        self.write_file_atomic(pipeline_name, &file)?;
        Ok(())
    }

    pub fn delete_step(&self, skills: &SkillsStore, pipeline_name: &str, step_id: i64) -> Result<()> {
        let mut file = self.read_file(pipeline_name, skills)?;
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
        skills: &SkillsStore,
        pipeline_name: &str,
        step_id: i64,
        target_step_id: i64,
    ) -> Result<()> {
        let mut file = self.read_file(pipeline_name, skills)?;
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
        skills: &SkillsStore,
        pipeline_name: &str,
        step_id: i64,
        skill_uuid: &str,
    ) -> Result<()> {
        let id = Uuid::parse_str(skill_uuid.trim())
            .map_err(|_| anyhow!("skill_id must be a valid UUID"))?;
        let index = skills.uuid_index()?;
        let Some(dir) = index.get(&id) else {
            return Err(anyhow!("skill not found"));
        };
        let alias = dir
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow!("skill not found"))?
            .to_string();

        let mut file = self.read_file(pipeline_name, skills)?;
        let Some(step) = file.steps.iter_mut().find(|s| s.id == step_id) else {
            return Err(anyhow!("step not found"));
        };
        if step.skills.iter().any(|s| s.id == id) {
            return Err(anyhow!("skill already attached to step"));
        }
        step.skills.push(PipelineSkillRef { id, alias });
        self.write_file_atomic(pipeline_name, &file)?;
        Ok(())
    }

    pub fn delete_step_skill(
        &self,
        skills: &SkillsStore,
        pipeline_name: &str,
        step_id: i64,
        skill_uuid: &str,
    ) -> Result<()> {
        let id = Uuid::parse_str(skill_uuid.trim())
            .map_err(|_| anyhow!("skill_id must be a valid UUID"))?;
        let mut file = self.read_file(pipeline_name, skills)?;
        let Some(step) = file.steps.iter_mut().find(|s| s.id == step_id) else {
            return Err(anyhow!("step not found"));
        };
        let before = step.skills.len();
        step.skills.retain(|s| s.id != id);
        if step.skills.len() == before {
            return Err(anyhow!("skill link not found"));
        }
        self.write_file_atomic(pipeline_name, &file)?;
        Ok(())
    }

    /// Update `alias` everywhere this skill id appears (after directory rename).
    pub fn update_alias_for_skill_id(
        &self,
        skills: &SkillsStore,
        skill_id: Uuid,
        new_alias: &str,
    ) -> Result<()> {
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
            let mut file = self.read_file(&name, skills)?;
            let mut changed = false;
            for step in &mut file.steps {
                for s in &mut step.skills {
                    if s.id == skill_id && s.alias != new_alias {
                        s.alias = new_alias.to_string();
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

    /// Remove skill id from all pipeline steps (skill deleted).
    pub fn remove_skill_id_everywhere(&self, skills: &SkillsStore, skill_id: Uuid) -> Result<()> {
        if !self.root.exists() {
            return Ok(());
        }
        let names = self.list_pipeline_names()?;
        for name in names {
            let mut file = self.read_file(&name, skills)?;
            let mut changed = false;
            for step in &mut file.steps {
                let before = step.skills.len();
                step.skills.retain(|s| s.id != skill_id);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    fn setup_skill(skills_root: &Path, name: &str, id: Uuid) {
        let dir = skills_root.join(name);
        fs::create_dir_all(&dir).expect("skill dir");
        fs::write(dir.join("SKILL.md"), "# test\n").expect("SKILL.md");
        fs::write(dir.join(crate::skills_store::SKILL_ID_FILE), format!("{id}\n"))
            .expect("skill id");
    }

    #[test]
    fn string_skill_ref_resolves_to_uuid_and_alias() {
        let temp = TempDir::new().expect("temp");
        let data = temp.path().join("data");
        let skills_root = temp.path().join("skills");
        let sid = Uuid::new_v4();
        setup_skill(&skills_root, "my-skill", sid);

        let store = PipelineStore::new(&data);
        let skills = SkillsStore::new(skills_root);
        fs::create_dir_all(data.join("pipelines/p1")).expect("pipe dir");
        fs::write(
            data.join("pipelines/p1/pipeline.json"),
            r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":["my-skill"]}]}"#,
        )
        .expect("pipeline.json");

        let steps = store.list_steps(&skills, "p1").expect("list_steps");
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].skills.len(), 1);
        assert_eq!(steps[0].skills[0].id, sid);
        assert_eq!(steps[0].skills[0].resolved_name.as_deref(), Some("my-skill"));
    }

    #[test]
    fn unknown_string_skill_ref_returns_error() {
        let temp = TempDir::new().expect("temp");
        let data = temp.path().join("data");
        let skills_root = temp.path().join("skills");
        fs::create_dir_all(&skills_root).expect("skills dir");

        let store = PipelineStore::new(&data);
        let skills = SkillsStore::new(skills_root);
        fs::create_dir_all(data.join("pipelines/p1")).expect("pipe dir");
        fs::write(
            data.join("pipelines/p1/pipeline.json"),
            r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":["missing-skill"]}]}"#,
        )
        .expect("pipeline.json");

        let err = store
            .list_steps(&skills, "p1")
            .expect_err("expected error");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("missing-skill") || msg.contains("not found"),
            "unexpected message: {msg}"
        );
    }

    #[test]
    fn mixed_string_and_object_skill_refs() {
        let temp = TempDir::new().expect("temp");
        let data = temp.path().join("data");
        let skills_root = temp.path().join("skills");
        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();
        setup_skill(&skills_root, "short-name", id_a);
        setup_skill(&skills_root, "other-skill", id_b);

        let store = PipelineStore::new(&data);
        let skills = SkillsStore::new(skills_root);
        fs::create_dir_all(data.join("pipelines/p1")).expect("pipe dir");
        let body = format!(
            r#"{{"steps":[{{"id":1,"title":"s","prompt":"p","skills":["short-name",{{"id":"{id_b}","alias":"other-skill"}}]}}]}}"#
        );
        fs::write(data.join("pipelines/p1/pipeline.json"), body).expect("pipeline.json");

        let steps = store.list_steps(&skills, "p1").expect("list_steps");
        assert_eq!(steps[0].skills.len(), 2);
        let ids: Vec<Uuid> = steps[0].skills.iter().map(|s| s.id).collect();
        assert!(ids.contains(&id_a));
        assert!(ids.contains(&id_b));
    }
}

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::PathBuf;

pub struct SkillsStore {
    root: PathBuf,
}

impl SkillsStore {
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn validate_name(name: &str) -> Result<()> {
        if name.is_empty() {
            bail!("skill name cannot be empty");
        }
        if !name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        {
            bail!("skill name must be alphanumeric, '-' or '_'");
        }
        Ok(())
    }

    #[must_use]
    pub fn skill_path(&self, name: &str) -> PathBuf {
        self.root.join(name).join("SKILL.md")
    }

    pub fn load_skill(&self, name: &str) -> Result<String> {
        let path = self.skill_path(name);
        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read skill '{}'", path.display()))?;
        Ok(content)
    }

    pub fn save_skill(&self, name: &str, content: &str) -> Result<()> {
        fs::create_dir_all(&self.root)
            .with_context(|| format!("failed to create skills dir '{}'", self.root.display()))?;
        let path = self.skill_path(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create skill dir '{}'", parent.display()))?;
        }
        fs::write(&path, content)
            .with_context(|| format!("failed to write skill '{}'", path.display()))?;
        Ok(())
    }

    pub fn delete_skill(&self, name: &str) -> Result<()> {
        let path = self.skill_path(name);
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("failed to delete skill '{}'", path.display()))?;
        }
        Ok(())
    }

    pub fn skill_exists(&self, name: &str) -> bool {
        self.skill_path(name).exists()
    }

    pub fn list_skill_names(&self) -> Result<Vec<String>> {
        if !self.root.exists() {
            return Ok(Vec::new());
        }
        let mut names = Vec::new();
        for entry in fs::read_dir(&self.root)
            .with_context(|| format!("failed to read skills dir '{}'", self.root.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let skill_path = path.join("SKILL.md");
            if !skill_path.exists() {
                continue;
            }
            if let Some(stem) = path.file_name().and_then(|value| value.to_str()) {
                names.push(stem.to_string());
            }
        }
        names.sort();
        Ok(names)
    }
}

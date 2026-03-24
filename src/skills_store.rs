use anyhow::{Context, Result, bail};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// File in each skill directory holding a single-line canonical UUID (v4).
pub const SKILL_ID_FILE: &str = ".prime-agent-skill-id";

#[derive(Clone)]
pub struct SkillsStore {
    root: PathBuf,
}

impl SkillsStore {
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn skill_id_path(skill_dir: &Path) -> PathBuf {
        skill_dir.join(SKILL_ID_FILE)
    }

    /// Normalize user input to a kebab-case slug (same rules as pipeline names).
    #[must_use]
    pub fn normalize_skill_name(name: &str) -> String {
        let s = name.trim().to_lowercase();
        let mut out = String::new();
        let mut prev_dash = false;
        for ch in s.chars() {
            let c = if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
                ch
            } else {
                '-'
            };
            if c == '-' {
                if !out.is_empty() && !prev_dash {
                    out.push('-');
                    prev_dash = true;
                }
            } else {
                out.push(c);
                prev_dash = false;
            }
        }
        while out.ends_with('-') {
            out.pop();
        }
        while out.starts_with('-') {
            out.remove(0);
        }
        out
    }

    pub fn validate_write_name(name: &str) -> Result<()> {
        if !name
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
            || name.is_empty()
        {
            bail!("name must contain only lowercase letters, digits, and dashes");
        }
        Ok(())
    }

    #[must_use]
    pub fn skill_path(&self, name: &str) -> PathBuf {
        self.root.join(name).join("SKILL.md")
    }

    fn write_skill_id_file(skill_dir: &Path, id: Uuid) -> Result<()> {
        let p = Self::skill_id_path(skill_dir);
        fs::write(&p, format!("{id}\n")).with_context(|| format!("write '{}'", p.display()))?;
        Ok(())
    }

    /// Ensure `SKILL_ID_FILE` exists and contains a valid UUID (lazy migration).
    pub fn ensure_skill_id_file(skill_dir: &Path) -> Result<Uuid> {
        let p = Self::skill_id_path(skill_dir);
        if p.is_file() {
            let raw = fs::read_to_string(&p)
                .with_context(|| format!("read skill id '{}'", p.display()))?;
            if let Ok(id) = Uuid::parse_str(raw.trim()) {
                return Ok(id);
            }
        }
        let id = Uuid::new_v4();
        Self::write_skill_id_file(skill_dir, id)?;
        Ok(id)
    }

    pub fn read_skill_id(skill_dir: &Path) -> Result<Uuid> {
        let p = Self::skill_id_path(skill_dir);
        let raw =
            fs::read_to_string(&p).with_context(|| format!("read skill id '{}'", p.display()))?;
        Uuid::parse_str(raw.trim()).with_context(|| format!("parse uuid in '{}'", p.display()))
    }

    /// Map skill UUID → skill directory path. Errors if the same UUID appears in two directories.
    pub fn uuid_index(&self) -> Result<HashMap<Uuid, PathBuf>> {
        let mut m = HashMap::new();
        for name in self.list_skill_names()? {
            let dir = self.root.join(&name);
            let id = Self::read_skill_id(&dir)?;
            if let Some(prev) = m.insert(id, dir.clone())
                && prev != dir
            {
                bail!(
                    "duplicate skill id {id} in '{}' and '{}'",
                    prev.display(),
                    dir.display()
                );
            }
        }
        Ok(m)
    }

    pub fn load_skill(&self, name: &str) -> Result<String> {
        let path = self.skill_path(name);
        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read skill '{}'", path.display()))?;
        if let Some(dir) = path.parent() {
            let _ = Self::ensure_skill_id_file(dir);
        }
        Ok(content)
    }

    pub fn save_skill(&self, name: &str, content: &str) -> Result<()> {
        Self::validate_write_name(name)?;
        fs::create_dir_all(&self.root)
            .with_context(|| format!("failed to create skills dir '{}'", self.root.display()))?;
        let path = self.skill_path(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create skill dir '{}'", parent.display()))?;
        }
        fs::write(&path, content)
            .with_context(|| format!("failed to write skill '{}'", path.display()))?;
        if let Some(dir) = path.parent() {
            Self::ensure_skill_id_file(dir)?;
        }
        Ok(())
    }

    pub fn rename_skill_directory(&self, old: &str, new: &str) -> Result<()> {
        Self::validate_write_name(new)?;
        let old_d = self.root.join(old);
        let new_d = self.root.join(new);
        if !old_d.is_dir() {
            bail!("skill directory not found");
        }
        if new_d.exists() {
            bail!("skill already exists");
        }
        fs::rename(&old_d, &new_d).with_context(|| {
            format!(
                "rename skill dir '{}' -> '{}'",
                old_d.display(),
                new_d.display()
            )
        })?;
        Ok(())
    }

    pub fn delete_skill(&self, name: &str) -> Result<()> {
        let dir = self.root.join(name);
        if dir.is_dir() {
            fs::remove_dir_all(&dir)
                .with_context(|| format!("failed to remove skill dir '{}'", dir.display()))?;
            return Ok(());
        }
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
                let _ = Self::ensure_skill_id_file(&path);
                names.push(stem.to_string());
            }
        }
        names.sort();
        Ok(names)
    }

    /// Read the stable id for a skill known by directory name.
    pub fn skill_uuid_for_name(&self, name: &str) -> Result<Uuid> {
        let dir = self.root.join(name);
        if !dir.is_dir() || !self.skill_path(name).exists() {
            bail!("skill not found");
        }
        Self::read_skill_id(&dir)
    }
}

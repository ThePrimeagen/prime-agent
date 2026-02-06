use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct Config {
    #[serde(rename = "skills-dir")]
    skills_dir: Option<PathBuf>,
    #[serde(flatten)]
    values: HashMap<String, String>,
}

impl Config {
    pub fn load_required(path: &Path) -> Result<Self> {
        if !path.exists() {
            bail!("config file missing at '{}'", path.display());
        }
        Self::load_from_path(path)
    }

    pub fn load_from_path(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("failed to read config '{}'", path.display()))?;
        let parsed: Self = serde_json::from_str(&contents)
            .with_context(|| format!("failed to parse config '{}'", path.display()))?;
        Ok(parsed)
    }

    pub fn load_or_default(path: &Path) -> Result<Self> {
        if path.exists() {
            Self::load_from_path(path)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save_to_path(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create '{}'", parent.display()))?;
        }
        let serialized = serde_json::to_string_pretty(self)
            .context("failed to serialize config")?;
        fs::write(path, format!("{serialized}\n"))
            .with_context(|| format!("failed to write config '{}'", path.display()))?;
        Ok(())
    }

    pub fn skills_dir(&self) -> Option<PathBuf> {
        self.skills_dir
            .clone()
            .map(|path| expand_path(&path))
    }

    pub fn set_value(&mut self, name: &str, value: &str) {
        if name == "skills-dir" {
            self.skills_dir = Some(expand_path(&PathBuf::from(value)));
        } else {
            self.values.insert(name.to_string(), value.to_string());
        }
    }

    pub fn get_value(&self, name: &str) -> Option<String> {
        if name == "skills-dir" {
            return self.skills_dir.as_ref().map(|path| path.display().to_string());
        }
        self.values.get(name).cloned()
    }

    pub fn all_values(&self) -> BTreeMap<String, String> {
        let mut values = BTreeMap::new();
        if let Some(path) = &self.skills_dir {
            values.insert("skills-dir".to_string(), path.display().to_string());
        }
        for (key, value) in &self.values {
            values.insert(key.clone(), value.clone());
        }
        values
    }

    pub fn apply_overrides(&mut self, overrides: &HashMap<String, String>) {
        for (key, value) in overrides {
            self.set_value(key, value);
        }
    }
}

pub fn ensure_config_file(path: &Path) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    Config::default().save_to_path(path)
}

pub fn config_path() -> Result<PathBuf> {
    if cfg!(target_os = "windows") {
        bail!("Microslop skill issues");
    }
    if let Ok(base) = env::var("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(base).join("prime-agent").join("config"));
    }
    if let Ok(home) = env::var("HOME") {
        if cfg!(target_os = "macos") {
            return Ok(PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("prime-agent")
                .join("config"));
        }
        return Ok(PathBuf::from(home)
            .join(".config")
            .join("prime-agent")
            .join("config"));
    }
    bail!("HOME not set and XDG_CONFIG_HOME not set");
}

fn expand_path(path: &Path) -> PathBuf {
    let raw = path.to_string_lossy();
    if (raw.starts_with("~/") || raw == "~")
        && let Ok(home) = env::var("HOME")
    {
        let suffix = raw.strip_prefix("~").unwrap_or("");
        return PathBuf::from(home).join(suffix.trim_start_matches('/'));
    }
    if raw.contains("$HOME")
        && let Ok(home) = env::var("HOME")
    {
        let replaced = raw.replace("$HOME", &home);
        return PathBuf::from(replaced);
    }
    path.to_path_buf()
}


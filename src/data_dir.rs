//! Resolve the prime-agent data directory (`skills/`, `pipelines/`, etc.).

use anyhow::Result;
use std::env;
use std::path::{Path, PathBuf};

use crate::config;

/// `PRIME_AGENT_DATA_DIR`, else `--data-dir`, else parent of the global config file (`…/prime-agent`).
pub fn resolve_data_dir(cli_data_dir: Option<&Path>) -> Result<PathBuf> {
    if let Ok(p) = env::var("PRIME_AGENT_DATA_DIR") {
        return Ok(PathBuf::from(p));
    }
    if let Some(p) = cli_data_dir {
        return Ok(expand_path(p));
    }
    let cfg = config::config_path()?;
    cfg.parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow::anyhow!("config path has no parent"))
}

fn expand_path(path: &Path) -> PathBuf {
    let raw = path.to_string_lossy();
    if (raw.starts_with("~/") || raw == "~") && let Ok(home) = env::var("HOME") {
        let suffix = raw.strip_prefix("~").unwrap_or("");
        return PathBuf::from(home).join(suffix.trim_start_matches('/'));
    }
    if raw.contains("$HOME") && let Ok(home) = env::var("HOME") {
        return PathBuf::from(raw.replace("$HOME", &home));
    }
    path.to_path_buf()
}

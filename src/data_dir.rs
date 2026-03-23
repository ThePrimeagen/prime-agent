//! Resolve the prime-agent data directory (`skills/`, `pipelines/`, etc.).

use anyhow::{Context, Result};
use std::env;
use std::path::{Path, PathBuf};

/// [`Cli::data_dir`](crate::cli::Cli::data_dir) when set (tilde / `$HOME` expanded), otherwise the
/// current working directory. No environment-variable or global-config fallbacks.
pub fn resolve_data_dir(cli_data_dir: Option<&Path>) -> Result<PathBuf> {
    if let Some(p) = cli_data_dir {
        return Ok(expand_path(p));
    }
    env::current_dir().context("current_dir for default data directory")
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
        return PathBuf::from(raw.replace("$HOME", &home));
    }
    path.to_path_buf()
}

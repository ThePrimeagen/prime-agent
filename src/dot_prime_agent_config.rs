//! Merge local `.prime-agent/config.json` with global `prime-agent/config.json` (runner, model,
//! optional `data-dir`).

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use serde_json::Value;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct DotPrimeAgentConfig {
    pub model: String,
    pub clirunner: String,
    /// Reserved for future CLI display options (parsed from config; default 3).
    #[allow(dead_code)]
    pub stdout_lines: u32,
    /// When true, pass `--force` to `cursor-agent` (non-interactive command allowlist; default on).
    pub yolo: bool,
    /// Resolved from merged `data-dir` (local over global); [`None`] when unset (use cwd via
    /// [`crate::data_dir::resolve_data_dir`]).
    pub data_dir: Option<PathBuf>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawLayer {
    #[serde(default)]
    model: Option<Value>,
    #[serde(default)]
    clirunner: Option<String>,
    #[serde(default)]
    cli: Option<String>,
    #[serde(default)]
    stdout_lines: Option<Value>,
    #[serde(default)]
    yolo: Option<bool>,
    #[serde(rename = "data-dir", default)]
    data_dir: Option<Value>,
}

#[derive(Debug, Default, Clone)]
struct NormalizedLayer {
    model: Option<Value>,
    clirunner: Option<String>,
    stdout_lines: Option<Value>,
    yolo: Option<bool>,
    data_dir: Option<Value>,
}

fn pick_clirunner(layer: &RawLayer) -> Option<String> {
    layer
        .clirunner
        .as_ref()
        .filter(|s| !s.is_empty())
        .cloned()
        .or_else(|| {
            layer
                .cli
                .as_ref()
                .filter(|s| !s.is_empty())
                .cloned()
        })
}

fn normalize(raw: RawLayer) -> NormalizedLayer {
    let clirunner = pick_clirunner(&raw);
    NormalizedLayer {
        model: raw.model,
        clirunner,
        stdout_lines: raw.stdout_lines,
        yolo: raw.yolo,
        data_dir: raw.data_dir,
    }
}

fn merge_normalized(local: Option<NormalizedLayer>, global: Option<NormalizedLayer>) -> NormalizedLayer {
    let l = local.unwrap_or_default();
    let g = global.unwrap_or_default();
    NormalizedLayer {
        model: l.model.or(g.model),
        clirunner: l.clirunner.or(g.clirunner),
        stdout_lines: l.stdout_lines.or(g.stdout_lines),
        yolo: l.yolo.or(g.yolo),
        data_dir: l.data_dir.or(g.data_dir),
    }
}

fn read_optional_layer(path: &Path) -> Result<Option<RawLayer>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path).with_context(|| format!("read '{}'", path.display()))?;
    let parsed: RawLayer =
        serde_json::from_str(&raw).with_context(|| format!("parse '{}'", path.display()))?;
    Ok(Some(parsed))
}

fn merge_from_paths(
    local_path: Option<&Path>,
    global_path: Option<&Path>,
) -> Result<NormalizedLayer> {
    let global_layer = match global_path {
        None => None,
        Some(p) => read_optional_layer(p)?.map(normalize),
    };
    let local_layer = match local_path {
        None => None,
        Some(p) => read_optional_layer(p)?.map(normalize),
    };
    Ok(merge_normalized(local_layer, global_layer))
}

fn expand_path(path: &Path) -> PathBuf {
    let raw = path.to_string_lossy();
    if (raw.starts_with("~/") || raw == "~")
        && let Ok(home) = env::var("HOME")
    {
        let suffix = raw.strip_prefix("~").unwrap_or("");
        return PathBuf::from(home).join(suffix.trim_start_matches('/'));
    }
    if raw.contains("$HOME") && let Ok(home) = env::var("HOME") {
        return PathBuf::from(raw.replace("$HOME", &home));
    }
    path.to_path_buf()
}

fn resolve_data_dir_value(v: Option<&Value>, cwd: &Path) -> Result<Option<PathBuf>> {
    let Some(v) = v else {
        return Ok(None);
    };
    match v {
        Value::Null => Ok(None),
        Value::String(s) if s.is_empty() => Ok(None),
        Value::String(s) => {
            let expanded = expand_path(Path::new(s));
            let resolved = if expanded.is_absolute() {
                expanded
            } else {
                cwd.join(expanded)
            };
            Ok(Some(resolved))
        }
        x => bail!("'data-dir' must be a string or null (got {x})"),
    }
}

fn finalize_merged(merged: NormalizedLayer, cwd: &Path) -> Result<DotPrimeAgentConfig> {
    let model = match merged.model {
        Some(Value::String(s)) if !s.is_empty() => s,
        _ => bail!(
            "merged config must set a non-empty string \"model\" (from '.prime-agent/config.json' and/or global config)"
        ),
    };

    let clirunner = merged.clirunner.ok_or_else(|| {
        anyhow!(
            "merged config must set \"clirunner\" (or legacy \"cli\") (from '.prime-agent/config.json' and/or global config)"
        )
    })?;

    let stdout_lines = match merged.stdout_lines {
        None => 3_u32,
        Some(Value::Number(n)) => {
            let v = n
                .as_u64()
                .ok_or_else(|| anyhow!("'stdout_lines' must be a positive integer (got {n})"))?;
            u32::try_from(v).map_err(|_| anyhow!("'stdout_lines' value out of range"))?
        }
        Some(x) => bail!("'stdout_lines' must be a number (got {x})"),
    };
    if stdout_lines == 0 {
        bail!("'stdout_lines' must be at least 1 (got 0)");
    }

    let yolo = merged.yolo.unwrap_or(true);
    let data_dir = resolve_data_dir_value(merged.data_dir.as_ref(), cwd)?;

    Ok(DotPrimeAgentConfig {
        model,
        clirunner,
        stdout_lines,
        yolo,
        data_dir,
    })
}

/// Merge global (`XDG_CONFIG_HOME/prime-agent/config.json`) and local `.prime-agent/config.json`.
pub fn load_merged(local_path: &Path) -> Result<DotPrimeAgentConfig> {
    let cwd = env::current_dir().context("current_dir for merged config")?;
    let global = crate::config::dot_config_json_path()?;
    let merged = merge_from_paths(Some(local_path), Some(&global))?;
    finalize_merged(merged, &cwd)
}

/// Data directory from merged `data-dir` only (no `model` / `clirunner` validation). For `serve`
/// when the local scaffold may still have null model.
pub fn merged_data_dir_for_serve(local_path: &Path) -> Result<Option<PathBuf>> {
    let cwd = env::current_dir().context("current_dir for merged data-dir")?;
    let global = crate::config::dot_config_json_path()?;
    let merged = merge_from_paths(Some(local_path), Some(&global))?;
    resolve_data_dir_value(merged.data_dir.as_ref(), &cwd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn finalize_local_only(path: &Path) -> DotPrimeAgentConfig {
        let cwd = path.parent().unwrap().to_path_buf();
        let merged = merge_from_paths(Some(path), None).unwrap();
        finalize_merged(merged, &cwd).unwrap()
    }

    #[test]
    fn defaults_stdout_lines_when_absent() {
        let temp = TempDir::new().unwrap();
        let p = temp.path().join("c.json");
        let mut f = fs::File::create(&p).unwrap();
        writeln!(f, r#"{{"model":"m","clirunner":"cursor-agent"}}"#).unwrap();
        let c = finalize_local_only(&p);
        assert_eq!(c.stdout_lines, 3);
        assert!(c.yolo);
    }

    #[test]
    fn parses_yolo_true() {
        let temp = TempDir::new().unwrap();
        let p = temp.path().join("c.json");
        fs::write(
            &p,
            r#"{"model":"m","clirunner":"cursor-agent","yolo":true}"#,
        )
        .unwrap();
        let c = finalize_local_only(&p);
        assert!(c.yolo);
    }

    #[test]
    fn parses_yolo_false() {
        let temp = TempDir::new().unwrap();
        let p = temp.path().join("c.json");
        fs::write(
            &p,
            r#"{"model":"m","clirunner":"cursor-agent","yolo":false}"#,
        )
        .unwrap();
        let c = finalize_local_only(&p);
        assert!(!c.yolo);
    }

    #[test]
    fn parses_stdout_lines_five() {
        let temp = TempDir::new().unwrap();
        let p = temp.path().join("c.json");
        fs::write(
            &p,
            r#"{"model":"m","clirunner":"cursor-agent","stdout_lines":5}"#,
        )
        .unwrap();
        let c = finalize_local_only(&p);
        assert_eq!(c.stdout_lines, 5);
    }

    #[test]
    fn rejects_zero_stdout_lines() {
        let temp = TempDir::new().unwrap();
        let p = temp.path().join("c.json");
        fs::write(
            &p,
            r#"{"model":"m","clirunner":"cursor-agent","stdout_lines":0}"#,
        )
        .unwrap();
        let cwd = temp.path().to_path_buf();
        let merged = merge_from_paths(Some(&p), None).unwrap();
        assert!(finalize_merged(merged, &cwd).is_err());
    }

    #[test]
    fn rejects_string_stdout_lines() {
        let temp = TempDir::new().unwrap();
        let p = temp.path().join("c.json");
        fs::write(
            &p,
            r#"{"model":"m","clirunner":"cursor-agent","stdout_lines":"3"}"#,
        )
        .unwrap();
        let cwd = temp.path().to_path_buf();
        let merged = merge_from_paths(Some(&p), None).unwrap();
        assert!(finalize_merged(merged, &cwd).is_err());
    }

    #[test]
    fn global_fills_model_when_local_null() {
        let temp = TempDir::new().unwrap();
        let local = temp.path().join("local.json");
        fs::write(&local, r#"{"model":null,"clirunner":"cursor-agent"}"#).unwrap();
        let global = temp.path().join("global.json");
        fs::write(
            &global,
            r#"{"model":"from-global","clirunner":"cursor-agent"}"#,
        )
        .unwrap();
        let cwd = temp.path().to_path_buf();
        let merged = merge_from_paths(Some(&local), Some(&global)).unwrap();
        let c = finalize_merged(merged, &cwd).unwrap();
        assert_eq!(c.model, "from-global");
    }

    #[test]
    fn local_overrides_global_yolo() {
        let temp = TempDir::new().unwrap();
        let local = temp.path().join("local.json");
        fs::write(
            &local,
            r#"{"model":"m","clirunner":"cursor-agent","yolo":false}"#,
        )
        .unwrap();
        let global = temp.path().join("global.json");
        fs::write(
            &global,
            r#"{"model":"m","clirunner":"cursor-agent","yolo":true}"#,
        )
        .unwrap();
        let cwd = temp.path().to_path_buf();
        let merged = merge_from_paths(Some(&local), Some(&global)).unwrap();
        let c = finalize_merged(merged, &cwd).unwrap();
        assert!(!c.yolo);
    }

    #[test]
    fn data_dir_local_over_global() {
        let temp = TempDir::new().unwrap();
        let local = temp.path().join("local.json");
        fs::write(
            &local,
            r#"{"model":"m","clirunner":"cursor-agent","data-dir":"loc"}"#,
        )
        .unwrap();
        let global = temp.path().join("global.json");
        fs::write(
            &global,
            r#"{"model":"m","clirunner":"cursor-agent","data-dir":"glob"}"#,
        )
        .unwrap();
        let cwd = temp.path().to_path_buf();
        let merged = merge_from_paths(Some(&local), Some(&global)).unwrap();
        let c = finalize_merged(merged, &cwd).unwrap();
        assert_eq!(c.data_dir, Some(cwd.join("loc")));
    }

    #[test]
    fn deny_unknown_field_errors() {
        let temp = TempDir::new().unwrap();
        let p = temp.path().join("c.json");
        fs::write(
            &p,
            r#"{"model":"m","clirunner":"cursor-agent","skills-dir":"/x"}"#,
        )
        .unwrap();
        assert!(read_optional_layer(&p).is_err());
    }
}

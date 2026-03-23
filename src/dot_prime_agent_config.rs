//! Load `.prime-agent/config.json` from the current working directory (runner + model).

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use serde_json::Value;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct DotPrimeAgentConfig {
    pub model: String,
    pub clirunner: String,
    /// Reserved for future CLI display options (parsed from config; default 3).
    #[allow(dead_code)]
    pub stdout_lines: u32,
    /// When true, pass `--force` to `cursor-agent` (non-interactive command allowlist; default on).
    pub yolo: bool,
}

#[derive(Debug, Deserialize)]
struct RawDotConfig {
    model: Option<Value>,
    #[serde(default)]
    clirunner: Option<String>,
    #[serde(default)]
    cli: Option<String>,
    #[serde(default)]
    stdout_lines: Option<Value>,
    #[serde(default)]
    yolo: Option<bool>,
}

pub fn load(path: &Path) -> Result<DotPrimeAgentConfig> {
    if !path.exists() {
        bail!(
            "missing '{}'; create it or run `prime-agent serve` once",
            path.display()
        );
    }
    let raw = fs::read_to_string(path).with_context(|| format!("read '{}'", path.display()))?;
    let parsed: RawDotConfig =
        serde_json::from_str(&raw).with_context(|| format!("parse '{}'", path.display()))?;

    let model = match parsed.model {
        Some(Value::String(s)) if !s.is_empty() => s,
        _ => bail!("'.prime-agent/config.json' must set a non-empty string \"model\""),
    };

    let clirunner = parsed
        .clirunner
        .filter(|s| !s.is_empty())
        .or(parsed.cli.filter(|s| !s.is_empty()))
        .ok_or_else(|| {
            anyhow!("'.prime-agent/config.json' must set \"clirunner\" (or legacy \"cli\")")
        })?;

    let stdout_lines = match parsed.stdout_lines {
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

    let yolo = parsed.yolo.unwrap_or(true);

    Ok(DotPrimeAgentConfig {
        model,
        clirunner,
        stdout_lines,
        yolo,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn defaults_stdout_lines_when_absent() {
        let temp = TempDir::new().unwrap();
        let p = temp.path().join("c.json");
        let mut f = fs::File::create(&p).unwrap();
        writeln!(f, r#"{{"model":"m","clirunner":"cursor-agent"}}"#).unwrap();
        let c = load(&p).unwrap();
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
        let c = load(&p).unwrap();
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
        let c = load(&p).unwrap();
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
        let c = load(&p).unwrap();
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
        assert!(load(&p).is_err());
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
        assert!(load(&p).is_err());
    }
}

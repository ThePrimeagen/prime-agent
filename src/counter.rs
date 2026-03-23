//! Persistent counter for `/fragments/counter`.

use anyhow::{Context, Result};
use serde_json::json;
use std::fs;
use std::path::Path;

pub fn increment_and_get(path: &Path) -> Result<i64> {
    let mut n = 0_i64;
    if path.exists() {
        let raw = fs::read_to_string(path).with_context(|| format!("read '{}'", path.display()))?;
        let v: serde_json::Value =
            serde_json::from_str(&raw).with_context(|| format!("parse '{}'", path.display()))?;
        n = v["count"].as_i64().unwrap_or(0);
    }
    n += 1;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create '{}'", parent.display()))?;
    }
    let payload = json!({ "count": n });
    let serialized = serde_json::to_string_pretty(&payload).context("serialize counter")?;
    fs::write(path, format!("{serialized}\n"))
        .with_context(|| format!("write '{}'", path.display()))?;
    Ok(n)
}

#![deny(warnings)]
#![warn(clippy::all, clippy::pedantic)]

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use std::env;
use std::path::{Path, PathBuf};

mod agents_md;
mod cli;
mod config;
mod skills_store;
mod sync;

use crate::agents_md::AgentSection;
use crate::cli::{Cli, Command, ConfigAction};
use crate::config::Config;
use crate::skills_store::SkillsStore;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let version = env!("CARGO_PKG_VERSION");
    println!("\u{001b}[32mprime-agent({version})\u{001b}[0m");

    let overrides = parse_config_overrides(&cli.config_overrides)?;

    if let Command::Config { action } = &cli.command {
        handle_config_command(action.as_ref())?;
        return Ok(());
    }

    let skills_dir = resolve_skills_dir(&cli, &overrides)?;
    let agents_path = cli
        .agents_path
        .unwrap_or_else(|| PathBuf::from("AGENTS.md"));
    let skills_store = SkillsStore::new(skills_dir);

    match cli.command {
        Command::Get { skills } => {
            let skill_names = cli::expand_skill_args(skills)?;
            let mut sections = Vec::with_capacity(skill_names.len());
            for name in skill_names {
                SkillsStore::validate_name(&name)?;
                let content = skills_store.load_skill(&name)?;
                sections.push(AgentSection::from_content(name, &content));
            }
            let rendered = agents_md::render_sections(&sections);
            std::fs::write(&agents_path, rendered)?;
        }
        Command::Set { name, path } => {
            SkillsStore::validate_name(&name)?;
            let content = std::fs::read_to_string(&path)?;
            skills_store.save_skill(&name, &content)?;
        }
        Command::Sync => {
            sync::run_sync(&skills_store, &agents_path)?;
        }
        Command::List => {
            for name in skills_store.list_skill_names()? {
                println!("{name}");
            }
        }
        Command::Config { .. } => {
            unreachable!("config command handled before skills setup");
        }
        Command::Delete { name } => {
            SkillsStore::validate_name(&name)?;
            let contents = std::fs::read_to_string(&agents_path)
                .with_context(|| format!("failed to read '{}'", agents_path.display()))?;
            let mut doc = agents_md::AgentsDoc::parse(&contents)?;
            if doc.remove_section(&name) {
                std::fs::write(&agents_path, doc.render())
                    .with_context(|| format!("failed to write '{}'", agents_path.display()))?;
            }
        }
        Command::DeleteGlobally { name } => {
            SkillsStore::validate_name(&name)?;
            let contents = std::fs::read_to_string(&agents_path)
                .with_context(|| format!("failed to read '{}'", agents_path.display()))?;
            let mut doc = agents_md::AgentsDoc::parse(&contents)?;
            if doc.remove_section(&name) {
                std::fs::write(&agents_path, doc.render())
                    .with_context(|| format!("failed to write '{}'", agents_path.display()))?;
            }
            skills_store.delete_skill(&name)?;
        }
    }
    Ok(())
}

fn handle_config_command(action: Option<&ConfigAction>) -> Result<()> {
    let path = config::config_path()?;
    config::ensure_config_file(&path)?;
    match action {
        Some(ConfigAction::Set { name, value }) => {
            let mut config = Config::load_or_default(&path)?;
            config.set_value(name, value);
            config.save_to_path(&path)?;
            print_config_with_updated(&config, name);
        }
        Some(ConfigAction::Get { name }) => {
            let config = Config::load_required(&path)?;
            if let Some(value) = config.get_value(name) {
                println!("{value}");
            } else {
                return Err(anyhow!("config value '{name}' not found"));
            }
        }
        None => {
            let config = Config::load_required(&path)?;
            print_config(&config);
        }
    }
    Ok(())
}

fn resolve_skills_dir(
    cli: &Cli,
    overrides: &std::collections::HashMap<String, String>,
) -> Result<PathBuf> {
    if let Some(path) = overrides.get("skills-dir").map(PathBuf::from) {
        return Ok(path);
    }
    if let Some(path) = cli.skills_dir.clone() {
        return Ok(expand_path(&path));
    }
    if let Ok(env_path) = env::var("PRIME_AGENT_SKILLS_DIR") {
        return Ok(expand_path(Path::new(&env_path)));
    }
    let config_path = config::config_path()?;
    let mut config = if config_path.exists() {
        Config::load_required(&config_path)?
    } else {
        Config::default()
    };
    config.apply_overrides(overrides);
    config
        .skills_dir()
        .context("skills directory not configured; use --skills-dir or config file")
}

fn parse_config_overrides(values: &[String]) -> Result<std::collections::HashMap<String, String>> {
    let mut overrides = std::collections::HashMap::new();
    for value in values {
        let Some((key, raw_value)) = value.split_once(':') else {
            return Err(anyhow!("invalid --config value '{value}', expected key:value"));
        };
        if key.trim().is_empty() {
            return Err(anyhow!("invalid --config value '{value}', empty key"));
        }
        let normalized = if key.trim() == "skills-dir" {
            expand_path(Path::new(raw_value))
                .to_string_lossy()
                .to_string()
        } else {
            raw_value.to_string()
        };
        overrides.insert(key.trim().to_string(), normalized);
    }
    Ok(overrides)
}

fn print_config(config: &Config) {
    let values = config.all_values();
    println!("Required:");
    let skills_dir = values
        .get("skills-dir")
        .map_or_else(|| "<missing>".to_string(), Clone::clone);
    println!("skills-dir={skills_dir}");
    println!("Optional:");
    for (key, value) in values {
        if key == "skills-dir" {
            continue;
        }
        println!("{key}={value}");
    }
}

fn print_config_with_updated(config: &Config, updated_key: &str) {
    let values = config.all_values();
    println!("Required:");
    let skills_dir = values
        .get("skills-dir")
        .map_or_else(|| "<missing>".to_string(), Clone::clone);
    if updated_key == "skills-dir" {
        println!("skills-dir={skills_dir} (updated)");
    } else {
        println!("skills-dir={skills_dir}");
    }
    println!("Optional:");
    for (key, value) in values {
        if key == "skills-dir" {
            continue;
        }
        if key == updated_key {
            println!("{key}={value} (updated)");
        } else {
            println!("{key}={value}");
        }
    }
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

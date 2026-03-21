#![deny(warnings)]
#![warn(clippy::all, clippy::pedantic)]

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use std::env;
use std::path::{Path, PathBuf};

mod agents_md;
mod cli;
mod config;
mod counter;
mod data_dir;
mod pipeline_store;
mod serve;
mod skills_store;
mod sync;
mod web;

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

    if let Command::Serve { bind } = &cli.command {
        let data_dir = crate::data_dir::resolve_data_dir(cli.data_dir.as_deref())?;
        let bind = bind
            .clone()
            .unwrap_or_else(|| "127.0.0.1:8080".to_string());
        serve::run_blocking(data_dir, bind)?;
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
            SkillsStore::validate_write_name(&name)?;
            let content = std::fs::read_to_string(&path)?;
            skills_store.save_skill(&name, &content)?;
        }
        Command::Sync => run_sync_cmd(&skills_store, &agents_path)?,
        Command::SyncRemote => run_sync_remote_cmd(&skills_store, &agents_path)?,
        Command::List { fragment } => run_list_cmd(&skills_store, fragment)?,
        Command::Local => run_local_cmd(&skills_store, &agents_path)?,
        Command::Config { .. } => {
            unreachable!("config command handled before skills setup");
        }
        Command::Serve { .. } => {
            unreachable!("serve command handled before skills setup");
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

fn run_sync_cmd(skills_store: &SkillsStore, agents_path: &Path) -> Result<()> {
    sync::run_sync(skills_store, agents_path)
}

fn run_sync_remote_cmd(skills_store: &SkillsStore, agents_path: &Path) -> Result<()> {
    sync::run_sync_remote(skills_store, agents_path)
}

fn run_list_cmd(skills_store: &SkillsStore, fragment: Option<String>) -> Result<()> {
    let mut skills = skills_store.list_skill_names()?;
    if let Some(fragment) = fragment {
        skills.retain(|name| name.contains(&fragment));
        println!("{}", skills.join(" "));
    } else {
        let mut first = true;
        for name in skills {
            if !first {
                println!();
            }
            first = false;
            println!("{name}");
        }
    }
    Ok(())
}

fn run_local_cmd(skills_store: &SkillsStore, agents_path: &Path) -> Result<()> {
    let agents_doc = if agents_path.exists() {
        let contents = std::fs::read_to_string(agents_path)
            .with_context(|| format!("failed to read '{}'", agents_path.display()))?;
        Some(agents_md::AgentsDoc::parse(&contents)?)
    } else {
        None
    };
    let Some(doc) = agents_doc.as_ref() else {
        return Ok(());
    };
    let section_names = doc.section_names();
    if section_names.is_empty() {
        return Ok(());
    }
    let statuses = sync::compute_sync_status(skills_store, agents_doc.as_ref())?;
    for name in section_names {
        match statuses.get(&name) {
            Some(sync::SyncStatus::Local) => {
                println!("{name} (out of sync: local)");
            }
            Some(sync::SyncStatus::Conflict) => {
                println!("{name} (out of sync: conflict)");
            }
            Some(sync::SyncStatus::Remote) => {
                println!("{name} (out of sync: remote)");
            }
            _ => {
                println!("{name}");
            }
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
            let resolved = resolve_config_value(name, value)?;
            config.set_value(name, &resolved);
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
    if let Some(dir) = config.skills_dir() {
        return Ok(dir);
    }
    let data_dir = crate::data_dir::resolve_data_dir(cli.data_dir.as_deref())?;
    Ok(data_dir.join("skills"))
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
        let normalized = resolve_config_value(key.trim(), raw_value)?;
        overrides.insert(key.trim().to_string(), normalized);
    }
    Ok(overrides)
}

fn resolve_config_value(key: &str, raw_value: &str) -> Result<String> {
    if key == "skills-dir" {
        let expanded = expand_path(Path::new(raw_value));
        let resolved = if expanded.is_absolute() {
            expanded
        } else {
            let cwd = std::env::current_dir()
                .context("failed to resolve current directory for skills-dir")?;
            cwd.join(expanded)
        };
        if let Ok(canonical) = resolved.canonicalize() {
            return Ok(canonical.to_string_lossy().to_string());
        }
        return Ok(resolved.to_string_lossy().to_string());
    }
    Ok(raw_value.to_string())
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

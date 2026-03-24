#![deny(warnings)]
#![warn(clippy::all, clippy::pedantic)]

use anyhow::{Context, Result, anyhow};
use clap::{CommandFactory, Parser};
use std::collections::HashMap;
use std::env;
use std::io::{BufRead, IsTerminal, Write, stdin, stdout};
use std::path::{Path, PathBuf};

mod agents_md;
mod cli;
mod config;
mod counter;
mod data_dir;
mod dot_prime_agent_config;
mod generation;
mod idle_commit;
mod live_reload;
mod pipeline_pick;
mod pipeline_progress;
mod pipeline_run;
mod pipeline_store;
mod serve;
mod skills_store;
mod sync;
mod web;

use crate::cli::{Cli, ConfigAction, RootCommand};
use crate::config::Config;
use crate::pipeline_run::PipelineRunOptions;
use crate::pipeline_store::PipelineStore;
use crate::skills_store::SkillsStore;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let version = env!("CARGO_PKG_VERSION");

    if let Some(RootCommand::Help) = &cli.command {
        Cli::command().print_long_help()?;
        return Ok(());
    }
    if let Some(RootCommand::Version) = &cli.command {
        println!("{version}");
        return Ok(());
    }

    println!("\u{001b}[32mprime-agent({version})\u{001b}[0m");

    let overrides = parse_config_overrides(&cli.config_overrides)?;
    print_effective_config_line(&cli, &overrides)?;

    if let Some(RootCommand::Config { action }) = &cli.command {
        handle_config_command(action.as_ref())?;
        return Ok(());
    }

    if let Some(RootCommand::Clear) = &cli.command {
        let cwd = std::env::current_dir().context("current_dir for clear")?;
        crate::pipeline_run::clear_pipeline_runs(&cwd)?;
        return Ok(());
    }

    if let Some(RootCommand::Serve { bind }) = &cli.command {
        let cwd = std::env::current_dir().context("current_dir for serve")?;
        let local_cfg = cwd.join(".prime-agent").join("config.json");
        let merged_dd =
            crate::dot_prime_agent_config::merged_data_dir_for_serve(&local_cfg)?;
        let data_dir = crate::data_dir::resolve_data_dir(
            cli.data_dir.as_deref(),
            merged_dd.as_deref(),
        )?;
        let bind = bind.clone().unwrap_or_else(|| "127.0.0.1:8080".to_string());
        serve::run_blocking(data_dir, bind)?;
        return Ok(());
    }

    if let Some(
        RootCommand::Run {
            name,
            prompt,
            file,
            no_tui: _,
        },
    ) = &cli.command
    {
        run_pipelines_command(
            &cli,
            &overrides,
            Some(&ExplicitRun {
                name: name.as_str(),
                prompt: prompt.as_deref(),
                file: file.as_ref(),
            }),
        )?;
        return Ok(());
    }

    if cli.command.is_none() {
        run_pipelines_command(&cli, &overrides, None)?;
        return Ok(());
    }

    let cwd = std::env::current_dir().context("current_dir for skills")?;
    let local_cfg = cwd.join(".prime-agent").join("config.json");
    let merged_dd = crate::dot_prime_agent_config::merged_data_dir_for_serve(&local_cfg)?;
    let skills_dir = resolve_skills_dir(&cli, &overrides, merged_dd.as_deref())?;
    let agents_path = cli
        .agents_path
        .clone()
        .unwrap_or_else(|| PathBuf::from("AGENTS.md"));
    let skills_store = SkillsStore::new(skills_dir);

    run_skills_commands(&cli, &skills_store, &agents_path)?;
    Ok(())
}

struct ExplicitRun<'a> {
    name: &'a str,
    prompt: Option<&'a str>,
    file: Option<&'a PathBuf>,
}

fn run_skills_commands(cli: &Cli, skills_store: &SkillsStore, agents_path: &Path) -> Result<()> {
    match &cli.command {
        Some(RootCommand::List { fragment }) => run_list_cmd(skills_store, fragment.clone())?,
        Some(RootCommand::Local) => run_local_cmd(skills_store, agents_path)?,
        _ => unreachable!("only list and local reach here"),
    }
    Ok(())
}

fn run_pipelines_command(
    cli: &Cli,
    overrides: &HashMap<String, String>,
    explicit_run: Option<&ExplicitRun<'_>>,
) -> Result<()> {
    let pipeline_flag = cli.pipeline.pipeline.as_deref();
    let prompt_flag = cli.pipeline.prompt.as_deref();
    let file_flag = cli.pipeline.file.as_ref();

    match explicit_run {
        Some(run) => {
            if pipeline_flag.is_some() {
                return Err(anyhow!(
                    "do not combine `run` with --pipeline; use `run <name>` or `--pipeline <name>` without the run subcommand"
                ));
            }
            let cwd = std::env::current_dir().context("current_dir for .prime-agent/config.json")?;
            let dot_path = cwd.join(".prime-agent").join("config.json");
            let dot = crate::dot_prime_agent_config::load_merged(&dot_path)?;
            let data_dir = crate::data_dir::resolve_data_dir(
                cli.data_dir.as_deref(),
                dot.data_dir.as_deref(),
            )?;
            let skills_dir = resolve_skills_dir(cli, overrides, dot.data_dir.as_deref())?;
            let user_text = match (run.prompt, run.file) {
                (Some(p), None) => p.to_string(),
                (None, Some(f)) => std::fs::read_to_string(f)
                    .with_context(|| format!("read user prompt file '{}'", f.display()))?,
                (None, None) => {
                    return Err(anyhow!(
                        "provide exactly one of --prompt or --file for `prime-agent run`"
                    ));
                }
                (Some(_), Some(_)) => {
                    return Err(anyhow!("use only one of --prompt or --file, not both"));
                }
            };
            let skills_store = SkillsStore::new(skills_dir);
            let options = PipelineRunOptions { debug: cli.debug };
            crate::pipeline_run::run(
                run.name,
                &user_text,
                &data_dir,
                &skills_store,
                &dot,
                &cwd,
                options,
            )?;
            Ok(())
        }
        None => {
            if let Some(pname) = pipeline_flag {
                let cwd = std::env::current_dir()
                    .context("current_dir for .prime-agent/config.json")?;
                let dot_path = cwd.join(".prime-agent").join("config.json");
                let dot = crate::dot_prime_agent_config::load_merged(&dot_path)?;
                let data_dir = crate::data_dir::resolve_data_dir(
                    cli.data_dir.as_deref(),
                    dot.data_dir.as_deref(),
                )?;
                let skills_dir = resolve_skills_dir(cli, overrides, dot.data_dir.as_deref())?;
                let user_text = match (prompt_flag, file_flag) {
                    (Some(p), None) => p.to_string(),
                    (None, Some(f)) => std::fs::read_to_string(f)
                        .with_context(|| format!("read user prompt file '{}'", f.display()))?,
                    (None, None) => {
                        return Err(anyhow!(
                            "with --pipeline, provide exactly one of --prompt or --file"
                        ));
                    }
                    (Some(_), Some(_)) => {
                        return Err(anyhow!("use only one of --prompt or --file, not both"));
                    }
                };
                let skills_store = SkillsStore::new(skills_dir);
                let options = PipelineRunOptions { debug: cli.debug };
                crate::pipeline_run::run(
                    pname,
                    &user_text,
                    &data_dir,
                    &skills_store,
                    &dot,
                    &cwd,
                    options,
                )?;
                Ok(())
            } else {
                run_pipelines_default(cli, overrides)
            }
        }
    }
}

fn run_pipelines_default(cli: &Cli, overrides: &HashMap<String, String>) -> Result<()> {
    let cwd = std::env::current_dir().context("current_dir for .prime-agent/config.json")?;
    let local_cfg = cwd.join(".prime-agent").join("config.json");
    let merged_dd = crate::dot_prime_agent_config::merged_data_dir_for_serve(&local_cfg)?;
    let data_dir = crate::data_dir::resolve_data_dir(
        cli.data_dir.as_deref(),
        merged_dd.as_deref(),
    )?;
    let store = PipelineStore::new(&data_dir);
    let skills_dir = resolve_skills_dir(cli, overrides, merged_dd.as_deref())?;
    let skills_store = SkillsStore::new(skills_dir);
    let entries = store.list_pipelines_with_health(&skills_store)?;
    if entries.is_empty() {
        eprintln!("No pipelines found.");
        return Ok(());
    }
    if !stdout().is_terminal() {
        print_pipeline_names(&entries);
        return Ok(());
    }

    let pipeline_name = pipeline_pick::pick_pipeline_interactive(&entries)?;
    let dot = crate::dot_prime_agent_config::load_merged(&local_cfg)?;
    let user_text = resolve_user_text_for_default_pipeline(cli)?;
    let options = PipelineRunOptions { debug: cli.debug };
    crate::pipeline_run::run(
        &pipeline_name,
        &user_text,
        &data_dir,
        &skills_store,
        &dot,
        &cwd,
        options,
    )
}

fn print_pipeline_names(entries: &[(String, bool)]) {
    let mut first = true;
    for (name, broken) in entries {
        if !first {
            println!();
        }
        first = false;
        if *broken {
            println!("{name} !");
        } else {
            println!("{name}");
        }
    }
}

fn read_user_prompt_line() -> Result<String> {
    print!("User prompt: ");
    stdout().flush().context("flush stdout")?;
    let mut line = String::new();
    stdin()
        .lock()
        .read_line(&mut line)
        .context("read user prompt from stdin")?;
    Ok(line.trim_end().to_string())
}

fn resolve_user_text_for_default_pipeline(cli: &Cli) -> Result<String> {
    match (cli.pipeline.prompt.as_deref(), cli.pipeline.file.as_ref()) {
        (Some(p), None) => Ok(p.to_string()),
        (None, Some(f)) => std::fs::read_to_string(f)
            .with_context(|| format!("read user prompt file '{}'", f.display())),
        (None, None) => read_user_prompt_line(),
        (Some(_), Some(_)) => Err(anyhow!("use only one of --prompt or --file, not both")),
    }
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

fn print_effective_config_line(cli: &Cli, overrides: &HashMap<String, String>) -> Result<()> {
    let cwd = std::env::current_dir().context("current_dir for effective config line")?;
    let local_cfg = cwd.join(".prime-agent").join("config.json");
    let merged_dd = crate::dot_prime_agent_config::merged_data_dir_for_serve(&local_cfg)?;
    let data_dir = crate::data_dir::resolve_data_dir(
        cli.data_dir.as_deref(),
        merged_dd.as_deref(),
    )?;
    let skills_dir = resolve_skills_dir(cli, overrides, merged_dd.as_deref())?;
    let line = crate::dot_prime_agent_config::format_effective_runtime_summary(
        &local_cfg,
        &data_dir,
        &skills_dir,
    );
    println!("{line}");
    Ok(())
}

fn resolve_skills_dir(
    cli: &Cli,
    overrides: &HashMap<String, String>,
    config_data_dir: Option<&Path>,
) -> Result<PathBuf> {
    if let Some(path) = overrides.get("skills-dir").map(PathBuf::from) {
        return Ok(path);
    }
    if let Some(path) = cli.skills_dir.clone() {
        return Ok(expand_path(&path));
    }
    let data_dir = crate::data_dir::resolve_data_dir(cli.data_dir.as_deref(), config_data_dir)?;
    Ok(data_dir.join("skills"))
}

fn parse_config_overrides(values: &[String]) -> Result<HashMap<String, String>> {
    let mut overrides = HashMap::new();
    for value in values {
        let Some((key, raw_value)) = value.split_once(':') else {
            return Err(anyhow!(
                "invalid --config value '{value}', expected key:value"
            ));
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn default_pipeline_both_prompt_and_file_errors() {
        let cli = Cli::parse_from([
            "prime-agent",
            "--prompt",
            "x",
            "--file",
            "/tmp/prime-agent-does-not-matter",
        ]);
        let err = resolve_user_text_for_default_pipeline(&cli).unwrap_err();
        assert!(
            err.to_string().contains("use only one of --prompt or --file"),
            "expected mutual exclusion error, got {err:?}"
        );
    }

    #[test]
    fn default_pipeline_prompt_only() {
        let cli = Cli::parse_from(["prime-agent", "--prompt", "hello"]);
        assert_eq!(
            resolve_user_text_for_default_pipeline(&cli).unwrap(),
            "hello"
        );
    }

    #[test]
    fn default_pipeline_file_only() {
        let temp = TempDir::new().expect("temp");
        let f = temp.path().join("p.txt");
        std::fs::write(&f, "from-file\n").expect("write");
        let cli = Cli::parse_from([
            "prime-agent",
            "--file",
            f.to_str().expect("utf8"),
        ]);
        assert_eq!(
            resolve_user_text_for_default_pipeline(&cli).unwrap(),
            "from-file\n"
        );
    }
}

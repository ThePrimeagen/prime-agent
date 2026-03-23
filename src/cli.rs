use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "prime-agent",
    version,
    about = "Skill-driven AGENTS.md builder and synchronizer"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
    /// Print debug messages to stderr (paths, stages, subprocess), and echo each cursor-agent
    /// stdout/stderr line as `{step}({n} / {total}):stdout|stderr:: ...`.
    #[arg(long, global = true)]
    pub debug: bool,
    /// Override configuration value (key:value). Can be repeated.
    #[arg(long = "config", value_name = "key:value")]
    pub config_overrides: Vec<String>,
    /// Prime-agent data directory (pipelines/, and default skills/ when `--skills-dir` is omitted).
    /// Default: current working directory (no global-config or environment fallbacks).
    #[arg(long, global = true)]
    pub data_dir: Option<PathBuf>,
    /// Directory containing skill markdown files (default: `<data-dir>/skills`, or `./skills` when
    /// `--data-dir` is omitted).
    #[arg(long)]
    pub skills_dir: Option<PathBuf>,
    /// Path to AGENTS.md (default: ./AGENTS.md)
    #[arg(long)]
    pub agents_path: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Build AGENTS.md from selected skills
    Get {
        /// Skill names (comma-separated or space-separated)
        skills: Vec<String>,
    },
    /// Store a skill markdown file under skills/<name>.md
    Set { name: String, path: PathBuf },
    /// Sync skills with AGENTS.md
    Sync,
    /// Sync skills and pull remote changes
    SyncRemote,
    /// List available skills
    List {
        /// Optional substring to filter skills
        fragment: Option<String>,
    },
    /// List local skills and sync status
    Local,
    /// Get or set configuration values
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },
    /// Run the web UI; static files are served from the current working directory
    Serve {
        /// Listen address (default: 127.0.0.1:8080)
        #[arg(long, env = "PRIME_AGENT_ADDR")]
        bind: Option<String>,
    },
    /// Remove a skill section from AGENTS.md
    Delete { name: String },
    /// Remove a skill section and delete its markdown file
    DeleteGlobally { name: String },
    /// Delete pipeline run artifacts under `./.prime-agent/pipelines/` in the current directory
    Clear,
    /// Pipelines: omit the subcommand to choose a pipeline from the interactive list (TTY) or list names (non-TTY); use `run` to invoke by name without the picker; or pass `--pipeline` with `--file` / `--prompt` to skip the picker and stdin prompt
    Pipelines {
        /// Pipeline to run (with `--file` or `--prompt`; omit `run` subcommand)
        #[arg(long)]
        pipeline: Option<String>,
        /// User prompt (with `--pipeline`; mutually exclusive with `--file`)
        #[arg(long)]
        prompt: Option<String>,
        /// Read user prompt from file (with `--pipeline`; mutually exclusive with `--prompt`)
        #[arg(long)]
        file: Option<PathBuf>,
        /// Ignored (pipelines always use plain stdout; kept for compatibility)
        #[arg(long)]
        no_tui: bool,
        #[command(subcommand)]
        action: Option<PipelinesAction>,
    },
}

#[derive(Subcommand, Debug)]
pub enum PipelinesAction {
    /// Execute pipeline stages; outputs under .prime-agent/pipelines/<adj-noun-slug>/
    Run {
        /// Pipeline name (kebab-case, must exist under data-dir/pipelines/)
        name: String,
        /// User prompt text (mutually exclusive with --file)
        #[arg(long)]
        prompt: Option<String>,
        /// Read user prompt from a UTF-8 file (mutually exclusive with --prompt)
        #[arg(long)]
        file: Option<PathBuf>,
        /// Ignored (kept for compatibility; use `PRIME_AGENT_NO_TUI=1` similarly)
        #[arg(long)]
        no_tui: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Set a configuration value
    Set { name: String, value: String },
    /// Get a configuration value
    Get { name: String },
}

pub fn expand_skill_args(args: Vec<String>) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for arg in args {
        for piece in arg.split(',') {
            let trimmed = piece.trim();
            if !trimmed.is_empty() {
                names.push(trimmed.to_string());
            }
        }
    }
    if names.is_empty() {
        bail!("no skills provided");
    }
    Ok(names)
}

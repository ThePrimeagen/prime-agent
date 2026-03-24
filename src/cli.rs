use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "prime-agent",
    version,
    about = "Pipelines, skills, and web UI for prime-agent",
    disable_help_subcommand = true
)]
pub struct Cli {
    #[command(flatten)]
    pub pipeline: PipelineRootArgs,
    #[command(subcommand)]
    pub command: Option<RootCommand>,
    /// Print debug messages to stderr (paths, stages, subprocess), and echo each cursor-agent
    /// stdout/stderr line as `{step}({n} / {total}):stdout|stderr:: ...`.
    #[arg(long, global = true)]
    pub debug: bool,
    /// Override configuration value (key:value). Can be repeated.
    #[arg(long = "config", value_name = "key:value")]
    pub config_overrides: Vec<String>,
    /// Prime-agent data directory (pipelines/, and default skills/ when `--skills-dir` is omitted).
    /// Default: merged `data-dir` from `.prime-agent/config.json` and global config, else cwd.
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

#[derive(clap::Args, Debug, Clone)]
pub struct PipelineRootArgs {
    /// Pipeline to run (with `--file` or `--prompt`; omit `run` subcommand)
    #[arg(long)]
    pub pipeline: Option<String>,
    /// User prompt (with `--pipeline` or default interactive run; mutually exclusive with `--file`)
    #[arg(long)]
    pub prompt: Option<String>,
    /// Read user prompt from file (with `--pipeline` or default interactive run; mutually exclusive
    /// with `--prompt`)
    #[arg(long)]
    pub file: Option<PathBuf>,
    /// Disable ratatui pipeline progress (plain stdout only). Same as `PRIME_AGENT_NO_TUI=1`.
    #[arg(long)]
    pub no_tui: bool,
}

#[derive(Subcommand, Debug)]
pub enum RootCommand {
    /// List available skills
    List {
        /// Optional substring to filter skills
        fragment: Option<String>,
    },
    /// List local skills and sync status
    Local,
    /// List, get, or set configuration values
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
    /// Delete pipeline run artifacts under `./.prime-agent/pipelines/` in the current directory
    Clear,
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
        /// Disable ratatui pipeline progress (plain stdout only).
        #[arg(long)]
        no_tui: bool,
    },
    /// Print this help message
    Help,
    /// Print version
    Version,
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Set a configuration value
    Set { name: String, value: String },
    /// Get a configuration value
    Get { name: String },
}

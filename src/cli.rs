use anyhow::{bail, Result};
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
    /// Override configuration value (key:value). Can be repeated.
    #[arg(long = "config", value_name = "key:value")]
    pub config_overrides: Vec<String>,
    /// Directory containing skill markdown files (default: ./skills)
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
    Set {
        name: String,
        path: PathBuf,
    },
    /// Sync skills with AGENTS.md
    Sync,
    /// Sync skills and pull remote changes
    SyncRemote,
    /// List available skills
    List,
    /// Get or set configuration values
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },
    /// Remove a skill section from AGENTS.md
    Delete {
        name: String,
    },
    /// Remove a skill section and delete its markdown file
    DeleteGlobally {
        name: String,
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

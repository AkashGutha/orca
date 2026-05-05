use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(author, version, about = "Agents orchestration platform")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Run commands from a config file.
    Run(RunArgs),
    /// Work toward a goal using configured agents.
    Goal(GoalArgs),
    /// Validate a config file without running commands.
    ValidateConfig(ConfigOnlyArgs),
    /// List command IDs from a config file in execution-plan order.
    List(ConfigOnlyArgs),
}

#[derive(Debug, Args)]
pub struct GoalArgs {
    /// Goal text to work on.
    #[arg(long, conflicts_with = "goal_file")]
    pub goal: Option<String>,

    /// Path to a Markdown file containing the goal.
    #[arg(long, value_name = "GOAL_FILE", conflicts_with = "goal")]
    pub goal_file: Option<PathBuf>,

    /// Path to the TOML/YAML config file.
    #[arg(short, long, alias = "config-file", value_name = "CONFIG_FILE")]
    pub config: Option<PathBuf>,

    /// Path to settings.toml.
    #[arg(long, value_name = "SETTINGS_FILE")]
    pub settings: Option<PathBuf>,

    /// Directory containing agent instruction overrides.
    #[arg(long)]
    pub instruction_dir: Option<PathBuf>,

    /// Directory where goal-run artifacts should be written.
    #[arg(long)]
    pub artifact_dir: Option<PathBuf>,

    /// Maximum number of agents to run in parallel.
    #[arg(long)]
    pub max_parallel_agents: Option<usize>,

    /// Approve the generated golden plan and continue to KPI/test/work agents.
    #[arg(long)]
    pub approve_golden_plan: bool,

    /// Emit JSON final summary.
    #[arg(long)]
    pub json: bool,

    /// Force plain line-oriented output instead of the live TUI.
    #[arg(long, conflicts_with = "tui")]
    pub plain: bool,

    /// Force the live terminal UI. If stdout is not a terminal, ORCA falls back to plain output.
    #[arg(long, conflicts_with = "plain")]
    pub tui: bool,
}

#[derive(Debug, Args)]
pub struct RunArgs {
    /// Path to the TOML/YAML config file.
    #[arg(short, long, alias = "config-file", value_name = "CONFIG_FILE")]
    pub config: Option<PathBuf>,

    /// Path to settings.toml.
    #[arg(long, value_name = "SETTINGS_FILE")]
    pub settings: Option<PathBuf>,

    /// Maximum number of commands to run at the same time.
    #[arg(short = 'j', long)]
    pub max_concurrency: Option<usize>,

    /// Continue scheduling independent commands after a command fails.
    #[arg(long, conflicts_with = "fail_fast")]
    pub continue_on_error: bool,

    /// Cancel running commands and stop scheduling new commands on first failure.
    #[arg(long, conflicts_with = "continue_on_error")]
    pub fail_fast: bool,

    /// Run only the selected command IDs and their dependencies.
    #[arg(long = "only", value_name = "COMMAND_ID")]
    pub only: Vec<String>,

    /// Emit JSON logs and final JSON summary.
    #[arg(long)]
    pub json: bool,
}

impl RunArgs {
    pub fn continue_on_error_override(&self) -> Option<bool> {
        if self.continue_on_error {
            Some(true)
        } else if self.fail_fast {
            Some(false)
        } else {
            None
        }
    }
}

#[derive(Debug, Args)]
pub struct ConfigOnlyArgs {
    /// Path to the TOML/YAML config file.
    #[arg(short, long, alias = "config-file", value_name = "CONFIG_FILE")]
    pub config: Option<PathBuf>,

    /// Path to settings.toml.
    #[arg(long, value_name = "SETTINGS_FILE")]
    pub settings: Option<PathBuf>,

    /// Emit JSON output.
    #[arg(long)]
    pub json: bool,
}

use std::fs;

use clap::Parser;
use orca_core::config::{
    AppConfig, default_orchestration_config, load_optional_config_with_settings,
};
use orca_core::errors::AppError;
use orca_core::executor::CommandExecutor;
use orca_core::goal::GoalRequest;
use orca_core::orchestrator::GoalOrchestrator;
use orca_core::planner::{PlanOptions, build_plan_from_source};
use orca_core::scheduler::Scheduler;
use orca_core::settings::Settings;

use crate::args::{Cli, Commands, GoalArgs, RunArgs};
use crate::logging::init_logging;
use crate::render::{OutputMode, OutputTask};

pub async fn run_from_env() -> Result<u8, AppError> {
    let args = normalize_goal_args(std::env::args());
    if args.len() == 1 {
        print_welcome_banner();
        return Ok(0);
    }

    run(Cli::parse_from(args)).await
}

pub async fn run(cli: Cli) -> Result<u8, AppError> {
    match cli.command {
        Commands::Run(args) => run_commands(args).await,
        Commands::Goal(args) => run_goal(args).await,
        Commands::ValidateConfig(args) => {
            init_logging(args.json)?;
            let settings = load_settings(args.settings.as_deref())?;
            validate_config_source(args.config.as_deref(), &settings)?;
            if args.json {
                println!("{}", serde_json::json!({ "valid": true }));
            } else {
                println!("config is valid");
            }
            Ok(0)
        }
        Commands::List(args) => {
            init_logging(args.json)?;
            let settings = load_settings(args.settings.as_deref())?;
            let options = PlanOptions::default();
            let plan = build_plan_from_source(args.config.as_deref(), &settings, options)?;
            if args.json {
                println!("{}", serde_json::to_string_pretty(&plan.commands)?);
            } else {
                for command in &plan.commands {
                    println!("{}", command.id);
                }
            }
            Ok(0)
        }
    }
}

fn normalize_goal_args<I>(args: I) -> Vec<String>
where
    I: IntoIterator<Item = String>,
{
    let mut args: Vec<_> = args.into_iter().collect();
    let Some(first) = args.get(1).cloned() else {
        return args;
    };

    let known_subcommands = ["run", "goal", "validate-config", "list", "help"];
    if known_subcommands.contains(&first.as_str())
        || first == "--help"
        || first == "-h"
        || first == "--version"
        || first == "-V"
    {
        return args;
    }

    if first == "--goal" || first == "--goal-file" {
        args.insert(1, "goal".to_string());
        return args;
    }

    if !first.starts_with('-') {
        let binary = args.remove(0);
        let option_start = args
            .iter()
            .position(|arg| arg.starts_with('-'))
            .unwrap_or(args.len());
        let rest = args.split_off(option_start);
        let goal = args.join(" ");
        let mut normalized = vec![binary, "goal".to_string(), "--goal".to_string(), goal];
        normalized.extend(rest);
        return normalized;
    }

    args
}

async fn run_goal(args: GoalArgs) -> Result<u8, AppError> {
    init_logging(args.json)?;
    let output_mode = output_mode(&args);
    let json = args.json;
    let request = goal_request_from_args(args)?;
    let output_task = OutputTask::start(output_mode);
    let result = GoalOrchestrator
        .run_with_output_handle(request, output_task.handle())
        .await;
    output_task.shutdown().await;
    let summary = result?;
    if json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        println!("{summary}");
    }
    Ok(summary.exit_code())
}

fn output_mode(args: &GoalArgs) -> OutputMode {
    if args.json {
        OutputMode::Json
    } else if args.plain {
        OutputMode::Plain
    } else if args.tui {
        OutputMode::Tui
    } else {
        OutputMode::Auto
    }
}

fn goal_request_from_args(args: GoalArgs) -> Result<GoalRequest, AppError> {
    let settings = load_settings(args.settings.as_deref())?;
    let goal = match (args.goal, args.goal_file) {
        (Some(goal), None) if !goal.trim().is_empty() => goal,
        (None, Some(path)) => {
            fs::read_to_string(&path).map_err(|source| AppError::ReadGoal { path, source })?
        }
        _ => {
            return Err(AppError::InvalidGoal(
                "provide non-empty --goal or --goal-file".to_string(),
            ));
        }
    };

    if goal.trim().is_empty() {
        return Err(AppError::InvalidGoal("goal must not be empty".to_string()));
    }

    Ok(GoalRequest {
        goal,
        config: args.config,
        settings,
        instruction_dir: args.instruction_dir,
        artifact_dir: args.artifact_dir,
        max_parallel_agents: args.max_parallel_agents,
        approve_golden_plan: args.approve_golden_plan,
        json: args.json,
    })
}

async fn run_commands(args: RunArgs) -> Result<u8, AppError> {
    init_logging(args.json)?;
    let settings = load_settings(args.settings.as_deref())?;
    let options = PlanOptions {
        max_concurrency: args.max_concurrency,
        continue_on_error: args.continue_on_error_override(),
        only: args.only,
    };
    let plan = build_plan_from_source(args.config.as_deref(), &settings, options)?;
    let summary = Scheduler::new(CommandExecutor).run(plan).await;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        println!("{summary}");
    }

    Ok(summary.exit_code())
}

fn load_settings(path: Option<&std::path::Path>) -> Result<Settings, AppError> {
    Settings::load_optional(path).map(|loaded| loaded.settings)
}

fn validate_config_source(
    path: Option<&std::path::Path>,
    settings: &Settings,
) -> Result<(), AppError> {
    let config = load_optional_config_with_settings(path, settings)?;
    if !config.commands.is_empty() {
        build_plan_from_config(config, PlanOptions::default())?;
        return Ok(());
    }

    let orchestration = config
        .orchestration
        .unwrap_or_else(default_orchestration_config);
    orchestration.into_agent_config()?;
    Ok(())
}

fn build_plan_from_config(
    config: AppConfig,
    options: PlanOptions,
) -> Result<orca_core::model::ExecutionPlan, AppError> {
    orca_core::planner::build_plan(config, options)
}

fn print_welcome_banner() {
    println!("{}", welcome_banner());
}

fn welcome_banner() -> &'static str {
    concat!(
        "\n",
        "+----------------------------------------------------------------+\n",
        "|            \x1b[38;2;255;48;0m ██████╗ ██████╗  ██████╗ █████╗ \x1b[0m               |\n",
        "|            \x1b[38;2;255;81;0m██╔═══██╗██╔══██╗██╔════╝██╔══██╗\x1b[0m               |\n",
        "|            \x1b[38;2;255;126;0m██║   ██║██████╔╝██║     ███████║\x1b[0m               |\n",
        "|            \x1b[38;2;255;162;0m██║   ██║██╔══██╗██║     ██╔══██║\x1b[0m               |\n",
        "|            \x1b[38;2;255;205;0m╚██████╔╝██║  ██║╚██████╗██║  ██║\x1b[0m               |\n",
        "|            \x1b[38;2;255;224;102m ╚═════╝ ╚═╝  ╚═╝ ╚═════╝╚═╝  ╚═╝\x1b[0m               |\n",
        "|                                                                |\n",
        "|              \x1b[1;38;2;255;205;0mORCA\x1b[0m - Agents orchestration platform              |\n",
        "+----------------------------------------------------------------+\n",
        "\n",
        "Set a goal to begin:\n",
        "\n",
        "  orca \"build the feature described in README.md\"\n",
        "  orca --goal \"fix the failing tests and update docs\"\n",
        "  orca --goal-file goal.md\n",
        "\n",
        "Useful options:\n",
        "\n",
        "  --plain    Use line-oriented output instead of the live dashboard\n",
        "  --tui      Force the live multi-agent dashboard\n",
        "  --json     Emit the final goal summary as JSON\n",
        "\n",
        "ORCA will plan, critique, generate a golden plan, run work/tests/KPI\n",
        "checks, and retry until the completion gates pass or max_iterations is hit.\n",
    )
}

#[cfg(test)]
mod tests {
    use super::{normalize_goal_args, print_welcome_banner, welcome_banner};

    #[test]
    fn treats_positional_text_as_goal() {
        let args = normalize_goal_args(
            ["orca", "ship", "feature", "--config", "x.toml"].map(str::to_string),
        );
        assert_eq!(
            args,
            vec![
                "orca",
                "goal",
                "--goal",
                "ship feature",
                "--config",
                "x.toml"
            ]
        );
    }

    #[test]
    fn inserts_goal_subcommand_for_goal_flag() {
        let args = normalize_goal_args(["orca", "--goal", "ship"].map(str::to_string));
        assert_eq!(args, vec!["orca", "goal", "--goal", "ship"]);
    }

    #[test]
    fn welcome_banner_is_printable() {
        print_welcome_banner();
    }

    #[test]
    fn welcome_banner_fits_standard_terminal_width() {
        assert!(
            welcome_banner()
                .lines()
                .all(|line| visible_banner_width(line) <= 80)
        );
        assert!(welcome_banner().contains("ORCA"));
        assert!(welcome_banner().contains("Agents orchestration platform"));
    }

    #[test]
    fn welcome_banner_uses_orca_terminal_theme() {
        assert!(welcome_banner().contains("\x1b[38;2;255;48;0m"));
        assert!(welcome_banner().contains("\x1b[1;38;2;255;205;0mORCA"));
    }

    fn visible_banner_width(line: &str) -> usize {
        let mut width = 0;
        let mut chars = line.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '\u{1b}' && chars.peek() == Some(&'[') {
                chars.next();
                for sequence_ch in chars.by_ref() {
                    if sequence_ch.is_ascii_alphabetic() {
                        break;
                    }
                }
                continue;
            }
            width += 1;
        }
        width
    }
}

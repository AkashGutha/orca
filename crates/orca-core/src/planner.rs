use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::time::Duration;

use crate::config::{AppConfig, CommandConfig, load_config};
use crate::errors::AppError;
use crate::model::{CommandSpec, ExecutionPlan};
use crate::settings::Settings;

const DEFAULT_MAX_CONCURRENCY: usize = 4;

#[derive(Debug, Clone, Default)]
pub struct PlanOptions {
    pub max_concurrency: Option<usize>,
    pub continue_on_error: Option<bool>,
    pub only: Vec<String>,
}

pub fn build_plan_from_path(path: &Path, options: PlanOptions) -> Result<ExecutionPlan, AppError> {
    let config = load_config(path)?;
    build_plan(config, options)
}

pub fn build_plan_from_source(
    path: Option<&Path>,
    settings: &Settings,
    options: PlanOptions,
) -> Result<ExecutionPlan, AppError> {
    let path = settings.resolve_workflow_config(path);
    build_plan_from_path(&path, options)
}

pub fn build_plan(config: AppConfig, options: PlanOptions) -> Result<ExecutionPlan, AppError> {
    if config.commands.is_empty() {
        return Err(AppError::InvalidConfig(
            "at least one [[commands]] entry is required".to_string(),
        ));
    }

    let max_concurrency = options
        .max_concurrency
        .or(config.max_concurrency)
        .unwrap_or(DEFAULT_MAX_CONCURRENCY);
    if max_concurrency == 0 {
        return Err(AppError::InvalidConfig(
            "max_concurrency must be greater than zero".to_string(),
        ));
    }

    validate_unique_ids(&config.commands)?;
    validate_command_options(&config.commands)?;
    validate_dependencies(&config.commands)?;
    validate_cycles(&config.commands)?;

    let selected = selected_command_ids(&config.commands, &options.only)?;
    let commands = config
        .commands
        .into_iter()
        .filter(|command| selected.contains(&command.id))
        .map(command_spec_from_config)
        .collect();

    Ok(ExecutionPlan {
        max_concurrency,
        continue_on_error: options
            .continue_on_error
            .or(config.continue_on_error)
            .unwrap_or(false),
        commands,
    })
}

fn command_spec_from_config(command: CommandConfig) -> CommandSpec {
    let timeout = command.timeout_secs.map(Duration::from_secs);
    CommandSpec {
        id: command.id,
        program: command.program,
        args: command.args,
        depends_on: command.depends_on,
        env: command.env,
        cwd: command.cwd,
        timeout,
        timeout_secs: command.timeout_secs,
        retries: command.retries,
        retry_delay_ms: command.retry_delay_ms,
        output: None,
        output_label: None,
        output_phase: None,
        output_model: None,
    }
}

fn validate_unique_ids(commands: &[CommandConfig]) -> Result<(), AppError> {
    let mut seen = HashSet::new();
    for command in commands {
        validate_command_id(&command.id)?;
        if command.program.trim().is_empty() {
            return Err(AppError::InvalidConfig(format!(
                "command `{}` must set a non-empty program",
                command.id
            )));
        }
        if !seen.insert(command.id.clone()) {
            return Err(AppError::InvalidConfig(format!(
                "duplicate command id `{}`",
                command.id
            )));
        }
    }
    Ok(())
}

fn validate_command_id(id: &str) -> Result<(), AppError> {
    if id.trim().is_empty() {
        return Err(AppError::InvalidConfig(
            "command id must not be empty".to_string(),
        ));
    }
    if id.chars().any(char::is_whitespace) {
        return Err(AppError::InvalidConfig(format!(
            "command id `{id}` must not contain whitespace"
        )));
    }
    Ok(())
}

fn validate_command_options(commands: &[CommandConfig]) -> Result<(), AppError> {
    for command in commands {
        if command.timeout_secs == Some(0) {
            return Err(AppError::InvalidConfig(format!(
                "command `{}` timeout_secs must be greater than zero",
                command.id
            )));
        }
        if let Some(cwd) = &command.cwd
            && !cwd.is_dir()
        {
            return Err(AppError::InvalidConfig(format!(
                "command `{}` cwd `{}` must be an existing directory",
                command.id,
                cwd.display()
            )));
        }
    }
    Ok(())
}

fn validate_dependencies(commands: &[CommandConfig]) -> Result<(), AppError> {
    let ids: HashSet<_> = commands.iter().map(|command| command.id.as_str()).collect();
    for command in commands {
        for dependency in &command.depends_on {
            if dependency == &command.id {
                return Err(AppError::InvalidConfig(format!(
                    "command `{}` cannot depend on itself",
                    command.id
                )));
            }
            if !ids.contains(dependency.as_str()) {
                return Err(AppError::InvalidConfig(format!(
                    "command `{}` depends on unknown command `{dependency}`",
                    command.id
                )));
            }
        }
    }
    Ok(())
}

fn validate_cycles(commands: &[CommandConfig]) -> Result<(), AppError> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Mark {
        Visiting,
        Visited,
    }

    fn visit(
        id: &str,
        by_id: &HashMap<&str, &CommandConfig>,
        marks: &mut HashMap<String, Mark>,
    ) -> Result<(), AppError> {
        match marks.get(id) {
            Some(Mark::Visited) => return Ok(()),
            Some(Mark::Visiting) => {
                return Err(AppError::InvalidConfig(format!(
                    "dependency cycle detected at `{id}`"
                )));
            }
            None => {}
        }

        marks.insert(id.to_string(), Mark::Visiting);
        for dependency in &by_id[id].depends_on {
            visit(dependency, by_id, marks)?;
        }
        marks.insert(id.to_string(), Mark::Visited);
        Ok(())
    }

    let by_id: HashMap<_, _> = commands
        .iter()
        .map(|command| (command.id.as_str(), command))
        .collect();
    let mut marks = HashMap::new();
    for command in commands {
        visit(&command.id, &by_id, &mut marks)?;
    }
    Ok(())
}

fn selected_command_ids(
    commands: &[CommandConfig],
    requested: &[String],
) -> Result<BTreeSet<String>, AppError> {
    let by_id: HashMap<_, _> = commands
        .iter()
        .map(|command| (command.id.as_str(), command))
        .collect();
    if requested.is_empty() {
        return Ok(commands.iter().map(|command| command.id.clone()).collect());
    }

    let mut selected = BTreeSet::new();
    for id in requested {
        if !by_id.contains_key(id.as_str()) {
            return Err(AppError::InvalidConfig(format!(
                "requested command `{id}` does not exist"
            )));
        }
        collect_with_dependencies(id, &by_id, &mut selected);
    }
    Ok(selected)
}

fn collect_with_dependencies(
    id: &str,
    by_id: &HashMap<&str, &CommandConfig>,
    selected: &mut BTreeSet<String>,
) {
    if !selected.insert(id.to_string()) {
        return;
    }
    for dependency in &by_id[id].depends_on {
        collect_with_dependencies(dependency, by_id, selected);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;

    fn command(id: &str, depends_on: Vec<&str>) -> CommandConfig {
        CommandConfig {
            id: id.to_string(),
            program: "true".to_string(),
            args: Vec::new(),
            depends_on: depends_on.into_iter().map(str::to_string).collect(),
            env: Default::default(),
            cwd: None,
            timeout_secs: None,
            retries: 0,
            retry_delay_ms: None,
        }
    }

    #[test]
    fn detects_dependency_cycles() {
        let config = AppConfig {
            max_concurrency: None,
            continue_on_error: None,
            orchestration: None,
            agents: None,
            commands: vec![command("a", vec!["b"]), command("b", vec!["a"])],
        };

        let err = build_plan(config, PlanOptions::default()).unwrap_err();
        assert!(err.to_string().contains("cycle"));
    }

    #[test]
    fn selected_commands_include_dependencies() {
        let config = AppConfig {
            max_concurrency: None,
            continue_on_error: None,
            orchestration: None,
            agents: None,
            commands: vec![
                command("setup", vec![]),
                command("build", vec!["setup"]),
                command("test", vec!["build"]),
            ],
        };

        let plan = build_plan(
            config,
            PlanOptions {
                only: vec!["test".to_string()],
                ..PlanOptions::default()
            },
        )
        .unwrap();

        let ids: Vec<_> = plan
            .commands
            .iter()
            .map(|command| command.id.as_str())
            .collect();
        assert_eq!(ids, vec!["setup", "build", "test"]);
    }
}

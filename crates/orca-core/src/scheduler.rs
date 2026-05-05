use std::collections::{BTreeMap, BTreeSet, HashMap};

use tokio::task::JoinSet;
use tracing::{info, warn};

use crate::executor::CommandExecutor;
use crate::model::{CommandResult, ExecutionPlan, RunSummary};

#[derive(Debug, Clone)]
pub struct Scheduler {
    executor: CommandExecutor,
}

impl Scheduler {
    pub fn new(executor: CommandExecutor) -> Self {
        Self { executor }
    }

    pub async fn run(&self, plan: ExecutionPlan) -> RunSummary {
        let mut pending: BTreeMap<_, _> = plan
            .commands
            .iter()
            .map(|command| (command.id.clone(), command.clone()))
            .collect();
        let mut results = HashMap::new();
        let mut ordered_results = Vec::new();
        let mut running = JoinSet::new();
        let mut running_ids: BTreeSet<String> = BTreeSet::new();
        let mut stopping = false;

        loop {
            if stopping {
                for id in pending.keys() {
                    ordered_results.push(CommandResult::skipped(
                        id.clone(),
                        "not started after earlier failure",
                    ));
                }
                pending.clear();

                for id in &running_ids {
                    ordered_results.push(CommandResult::cancelled(id.clone()));
                }
                running.abort_all();
                while running.join_next().await.is_some() {}
                break;
            }

            skip_commands_with_failed_dependencies(&mut pending, &results, &mut ordered_results);

            while running.len() < plan.max_concurrency {
                let Some(id) = next_ready_command(&pending, &results) else {
                    break;
                };
                let command = pending
                    .remove(&id)
                    .expect("ready command must exist in pending map");
                let executor = self.executor.clone();
                running_ids.insert(id.clone());
                info!(command_id = %id, "scheduling command");
                running.spawn(async move {
                    let result = executor.execute(command).await;
                    (id, result)
                });
            }

            if running.is_empty() {
                if pending.is_empty() {
                    break;
                }

                for id in pending.keys() {
                    ordered_results.push(CommandResult::skipped(
                        id.clone(),
                        "dependencies did not complete successfully",
                    ));
                }
                break;
            }

            match running.join_next().await {
                Some(Ok((id, result))) => {
                    running_ids.remove(&id);
                    if result.failed() && !plan.continue_on_error {
                        warn!(command_id = %id, "command failed; cancelling run");
                        stopping = true;
                    }
                    results.insert(id, result.clone());
                    ordered_results.push(result);
                }
                Some(Err(err)) => {
                    warn!(error = %err, "command task failed");
                    if !plan.continue_on_error {
                        stopping = true;
                    }
                }
                None => {}
            }
        }

        RunSummary {
            results: ordered_results,
        }
    }
}

fn next_ready_command(
    pending: &BTreeMap<String, crate::model::CommandSpec>,
    results: &HashMap<String, CommandResult>,
) -> Option<String> {
    pending
        .iter()
        .find(|(_, command)| {
            command.depends_on.iter().all(|dependency| {
                results
                    .get(dependency)
                    .is_some_and(CommandResult::succeeded)
            })
        })
        .map(|(id, _)| id.clone())
}

fn skip_commands_with_failed_dependencies(
    pending: &mut BTreeMap<String, crate::model::CommandSpec>,
    results: &HashMap<String, CommandResult>,
    ordered_results: &mut Vec<CommandResult>,
) {
    let skipped: Vec<_> = pending
        .iter()
        .filter(|(_, command)| {
            command
                .depends_on
                .iter()
                .any(|dependency| results.get(dependency).is_some_and(CommandResult::failed))
        })
        .map(|(id, _)| id.clone())
        .collect();

    for id in skipped {
        pending.remove(&id);
        ordered_results.push(CommandResult::skipped(
            id,
            "dependency failed or was skipped",
        ));
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::model::CommandSpec;

    use super::*;

    fn spec(id: &str, program: &str, depends_on: Vec<&str>) -> CommandSpec {
        CommandSpec {
            id: id.to_string(),
            program: program.to_string(),
            args: Vec::new(),
            depends_on: depends_on.into_iter().map(str::to_string).collect(),
            env: BTreeMap::new(),
            cwd: None,
            timeout: None,
            timeout_secs: None,
            retries: 0,
            retry_delay_ms: None,
            output: None,
            output_label: None,
            output_phase: None,
            output_model: None,
        }
    }

    #[tokio::test]
    async fn skips_dependents_after_failure_in_continue_mode() {
        let plan = ExecutionPlan {
            max_concurrency: 2,
            continue_on_error: true,
            commands: vec![
                spec("fail", "false", vec![]),
                spec("after", "true", vec!["fail"]),
            ],
        };

        let summary = Scheduler::new(CommandExecutor).run(plan).await;

        assert_eq!(summary.results.len(), 2);
        assert!(summary.results.iter().any(|result| result.id == "after"));
        assert_eq!(summary.exit_code(), 1);
    }
}

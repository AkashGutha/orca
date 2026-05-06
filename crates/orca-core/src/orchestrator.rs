use std::collections::{BTreeMap, HashSet};

use serde::Serialize;
use tokio::task::JoinSet;
use tracing::info;

use crate::agent::{AgentConfig, AgentInput, AgentOutput, AgentSpec, ApprovalMode};
use crate::agent_backend::{Agent, ExternalCommandAgent};
use crate::approval::ApprovalGate;
use crate::artifacts::ArtifactWorkspace;
use crate::completion::CompletionReport;
use crate::config::{default_orchestration_config, load_optional_config_with_settings};
use crate::errors::AppError;
use crate::goal::{GoalRequest, GoalSummary};
use crate::instructions::resolve_instruction;
use crate::output::{OutputHandle, OutputObserver};

#[derive(Debug, Clone, Default)]
pub struct GoalOrchestrator;

impl GoalOrchestrator {
    pub async fn run(&self, request: GoalRequest) -> Result<GoalSummary, AppError> {
        self.run_with_output_handle(request, None).await
    }

    pub async fn run_with_output_observer(
        &self,
        request: GoalRequest,
        observer: Option<OutputObserver>,
    ) -> Result<GoalSummary, AppError> {
        let output_handle = observer.map(OutputHandle::observer);
        self.run_with_output_handle(request, output_handle).await
    }

    pub async fn run_with_output_handle(
        &self,
        request: GoalRequest,
        output_handle: Option<OutputHandle>,
    ) -> Result<GoalSummary, AppError> {
        let config =
            load_optional_config_with_settings(request.config.as_deref(), &request.settings)?;
        let orchestration_config = config
            .orchestration
            .unwrap_or_else(default_orchestration_config);
        let agent_config = orchestration_config.into_agent_config()?;
        validate_agent_config(&agent_config)?;

        let instruction_dir = request.instruction_dir.as_deref();
        let agent_instruction_dir = agent_config.instruction_dir.as_deref();
        let settings_artifact_dir = request
            .artifact_dir
            .clone()
            .or_else(|| request.settings.default_artifact_dir());
        let artifact_dir = settings_artifact_dir
            .as_deref()
            .or(agent_config.artifact_dir.as_deref());
        let max_parallel = request
            .max_parallel_agents
            .or_else(|| request.settings.default_max_parallel_agents())
            .or(agent_config.max_parallel_agents)
            .unwrap_or(4);
        if max_parallel == 0 {
            return Err(AppError::InvalidConfig(
                "orchestration.max_parallel_agents must be greater than zero".to_string(),
            ));
        }

        let backend = ExternalCommandAgent::new(output_handle.clone());
        let workspace = ArtifactWorkspace::create(artifact_dir)?;
        workspace.append_event(&GoalEvent::new("goal_started", "goal workflow started"))?;

        let max_iterations = agent_config.max_iterations.unwrap_or(1).max(1);
        let approval_mode = agent_config.approval_mode.unwrap_or(ApprovalMode::Auto);
        let approved = ApprovalGate::new(approval_mode, request.approve_golden_plan).approved();
        let feedback_specs = agent_config
            .specs
            .iter()
            .filter(|spec| spec.output_contract == "feedback")
            .cloned()
            .collect::<Vec<_>>();
        let main_specs = agent_config
            .specs
            .iter()
            .filter(|spec| spec.output_contract != "feedback")
            .cloned()
            .collect::<Vec<_>>();

        let mut outputs = Vec::new();
        let mut golden_plan_path = None;
        let mut completed = false;
        let mut feedback = String::new();
        let mut iterations = 0;

        for iteration in 1..=max_iterations {
            iterations = iteration;
            workspace.append_event(&GoalEvent::new(
                "iteration_started",
                "goal iteration started",
            ))?;

            let graph = self
                .run_graph(
                    &backend,
                    GraphRun {
                        specs: main_specs.clone(),
                        goal: &request.goal,
                        feedback: &feedback,
                        gate_feedback: "",
                        instruction_dir,
                        agent_instruction_dir,
                        settings: &request.settings,
                        workspace: &workspace,
                        max_parallel,
                        iteration,
                        output: output_handle.as_ref(),
                        approved,
                    },
                )
                .await?;
            let mut iteration_outputs = graph.outputs;
            if golden_plan_path.is_none() {
                golden_plan_path = iteration_outputs
                    .iter()
                    .find(|output| output.output_contract == "golden_plan")
                    .map(|output| output.artifact_path.clone());
            }

            if graph.stopped_for_approval {
                outputs.extend(iteration_outputs);
                feedback = "golden plan requires approval".to_string();
                publish_iteration_summary(
                    output_handle.as_ref(),
                    iteration,
                    false,
                    false,
                    &feedback,
                    max_iterations,
                );
                break;
            }

            let report = CompletionReport::evaluate(&iteration_outputs);
            workspace.write_json(&format!("iteration-{iteration}/completion.json"), &report)?;
            completed = report.complete;

            let mut next_feedback = report.feedback.clone();
            if !completed && !feedback_specs.is_empty() {
                let feedback_outputs = self
                    .run_graph(
                        &backend,
                        GraphRun {
                            specs: feedback_specs.clone(),
                            goal: &request.goal,
                            feedback: &feedback,
                            gate_feedback: &report.feedback,
                            instruction_dir,
                            agent_instruction_dir,
                            settings: &request.settings,
                            workspace: &workspace,
                            max_parallel,
                            iteration,
                            output: output_handle.as_ref(),
                            approved: true,
                        },
                    )
                    .await?
                    .outputs;
                let generated_feedback = InputBundle::from_outputs(&feedback_outputs).render();
                if !generated_feedback.trim().is_empty() {
                    next_feedback = generated_feedback;
                }
                iteration_outputs.extend(feedback_outputs);
            }

            feedback = next_feedback;
            outputs.extend(iteration_outputs);

            publish_iteration_summary(
                output_handle.as_ref(),
                iteration,
                completed,
                iteration < max_iterations,
                &feedback,
                max_iterations,
            );

            if completed {
                workspace.append_event(&GoalEvent::new("goal_completed", "all gates passed"))?;
                break;
            }

            workspace.write_text(&format!("iteration-{iteration}/feedback.md"), &feedback)?;
            workspace.append_event(&GoalEvent::new(
                "iteration_feedback",
                "completion gates failed; feeding back into graph inputs",
            ))?;
        }

        let summary = GoalSummary {
            goal: request.goal,
            artifact_root: workspace.root().display().to_string(),
            approved,
            completed,
            iterations,
            feedback: if completed || feedback.trim().is_empty() {
                None
            } else {
                Some(feedback)
            },
            golden_plan_path,
            outputs,
            json: request.json,
        };
        workspace.write_json("manifest.json", &summary)?;
        workspace.append_event(&GoalEvent::new("goal_finished", "goal workflow finished"))?;
        Ok(summary)
    }

    async fn run_graph(
        &self,
        backend: &ExternalCommandAgent,
        run: GraphRun<'_>,
    ) -> Result<GraphRunResult, AppError> {
        let mut pending = run.specs.into_iter().enumerate().collect::<Vec<_>>();
        let mut running = JoinSet::new();
        let mut running_resources = HashSet::<String>::new();
        let mut completed = HashSet::<String>::new();
        let mut output_map = BTreeMap::<String, AgentOutput>::new();
        let mut outputs = Vec::<(usize, AgentOutput)>::new();
        let mut stopped_for_approval = false;

        loop {
            while running.len() < run.max_parallel {
                let Some(position) = pending.iter().position(|(_, spec)| {
                    dependencies_satisfied(spec, &completed)
                        && resources_available(spec, &running_resources)
                }) else {
                    break;
                };
                let (order, spec) = pending.remove(position);
                for resource in &spec.resources {
                    running_resources.insert(resource.clone());
                }

                if let Some(output) = run.output {
                    let label = spec.phase_label.as_deref().unwrap_or(&spec.output_contract);
                    output.phase_started(label);
                }

                let backend = backend.clone();
                let instruction_dirs = run
                    .settings
                    .instruction_sources(run.instruction_dir, run.agent_instruction_dir);
                let instruction = resolve_instruction(&spec, &instruction_dirs)?;
                let input = AgentInput {
                    goal: run.goal.to_string(),
                    context: build_context(
                        &spec,
                        run.goal,
                        run.feedback,
                        run.gate_feedback,
                        &output_map,
                    ),
                };
                record_agent_input(run.workspace, run.output, run.iteration, &spec, &input)?;
                info!(
                    agent_id = %spec.id,
                    output_contract = %spec.output_contract,
                    "starting graph node"
                );
                running.spawn(async move {
                    let resources = spec.resources.clone();
                    backend
                        .run(&spec, input, &instruction)
                        .await
                        .map(|output| (order, output, resources))
                });
            }

            match running.join_next().await {
                Some(Ok(Ok((order, output, resources)))) => {
                    for resource in resources {
                        running_resources.remove(&resource);
                    }
                    let persisted = run
                        .workspace
                        .write_agent_output_for_iteration(run.iteration, &output)?;
                    completed.insert(persisted.agent_id.clone());
                    output_map.insert(persisted.agent_id.clone(), persisted.clone());
                    if persisted.output_contract == "golden_plan" && !run.approved {
                        pending.clear();
                        stopped_for_approval = true;
                    }
                    outputs.push((order, persisted));
                }
                Some(Ok(Err(err))) => return Err(err),
                Some(Err(err)) => {
                    return Err(AppError::InvalidGoal(format!(
                        "agent task failed to join: {err}"
                    )));
                }
                None if pending.is_empty() => break,
                None => {
                    let blocked = pending
                        .iter()
                        .map(|(_, spec)| spec.id.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    return Err(AppError::InvalidConfig(format!(
                        "graph could not make progress; blocked nodes: {blocked}"
                    )));
                }
            }
        }

        outputs.sort_by_key(|(order, _)| *order);
        Ok(GraphRunResult {
            outputs: outputs.into_iter().map(|(_, output)| output).collect(),
            stopped_for_approval,
        })
    }
}

struct GraphRun<'a> {
    specs: Vec<AgentSpec>,
    goal: &'a str,
    feedback: &'a str,
    gate_feedback: &'a str,
    instruction_dir: Option<&'a std::path::Path>,
    agent_instruction_dir: Option<&'a std::path::Path>,
    settings: &'a crate::settings::Settings,
    workspace: &'a ArtifactWorkspace,
    max_parallel: usize,
    iteration: usize,
    output: Option<&'a OutputHandle>,
    approved: bool,
}

struct GraphRunResult {
    outputs: Vec<AgentOutput>,
    stopped_for_approval: bool,
}

#[derive(Debug, Clone)]
struct AgentConnectionInput {
    source_id: String,
    content: String,
}

#[derive(Debug, Clone, Default)]
struct InputBundle {
    inputs: Vec<AgentConnectionInput>,
}

impl InputBundle {
    fn from_outputs(outputs: &[AgentOutput]) -> Self {
        let inputs = outputs
            .iter()
            .map(|output| AgentConnectionInput {
                source_id: output.agent_id.clone(),
                content: output.content.clone(),
            })
            .collect();
        Self { inputs }
    }

    fn render(&self) -> String {
        self.inputs
            .iter()
            .filter(|input| !input.content.trim().is_empty())
            .map(|input| format!("## {}\n\n{}", input.source_id, input.content))
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

fn build_context(
    spec: &AgentSpec,
    goal: &str,
    feedback: &str,
    gate_feedback: &str,
    outputs: &BTreeMap<String, AgentOutput>,
) -> String {
    let mut bundle = InputBundle::default();
    for source in &spec.input_sources {
        match source.as_str() {
            "goal" => bundle.inputs.push(AgentConnectionInput {
                source_id: "goal".to_string(),
                content: goal.to_string(),
            }),
            "feedback" => bundle.inputs.push(AgentConnectionInput {
                source_id: "feedback".to_string(),
                content: feedback.to_string(),
            }),
            "gate-feedback" => bundle.inputs.push(AgentConnectionInput {
                source_id: "gate-feedback".to_string(),
                content: gate_feedback.to_string(),
            }),
            source => {
                if let Some(output) = outputs.get(source) {
                    bundle.inputs.push(AgentConnectionInput {
                        source_id: output.agent_id.clone(),
                        content: output.content.clone(),
                    });
                }
            }
        }
    }
    bundle.render()
}

fn dependencies_satisfied(spec: &AgentSpec, completed: &HashSet<String>) -> bool {
    spec.depends_on
        .iter()
        .all(|dependency| is_pseudo_source(dependency) || completed.contains(dependency))
}

fn resources_available(spec: &AgentSpec, running_resources: &HashSet<String>) -> bool {
    spec.resources
        .iter()
        .all(|resource| !running_resources.contains(resource))
}

fn is_pseudo_source(source: &str) -> bool {
    matches!(source, "goal" | "feedback" | "gate-feedback")
}

fn validate_agent_config(config: &AgentConfig) -> Result<(), AppError> {
    if config.specs.is_empty() {
        return Err(AppError::InvalidConfig(
            "orchestration.nodes must contain at least one agent node".to_string(),
        ));
    }
    for spec in &config.specs {
        if spec.id.trim().is_empty() {
            return Err(AppError::InvalidConfig(
                "agent id must not be empty".to_string(),
            ));
        }
        if spec.command.trim().is_empty() {
            return Err(AppError::InvalidConfig(format!(
                "agent `{}` command must not be empty",
                spec.id
            )));
        }
        if spec.output_contract.trim().is_empty() {
            return Err(AppError::InvalidConfig(format!(
                "agent `{}` output_contract must not be empty",
                spec.id
            )));
        }
    }
    Ok(())
}

fn record_agent_input(
    workspace: &ArtifactWorkspace,
    output: Option<&OutputHandle>,
    iteration: usize,
    spec: &AgentSpec,
    input: &AgentInput,
) -> Result<(), AppError> {
    workspace.append_event(&AgentInputEvent {
        event: "agent_input",
        iteration,
        agent_id: &spec.id,
        kind: format!("{:?}", spec.kind),
        output_contract: &spec.output_contract,
        input,
    })?;
    if let Some(output) = output {
        output.agent_input(&spec.id, &input_glimpse(input));
    }
    Ok(())
}

fn input_glimpse(input: &AgentInput) -> String {
    let context = first_feedback_line(&input.context).unwrap_or("");
    truncate_plain(&format!("goal: {} | context: {}", input.goal, context), 160)
}

fn truncate_plain(value: &str, max_chars: usize) -> String {
    let mut normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_chars {
        return normalized;
    }
    normalized = normalized
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect();
    normalized.push('…');
    normalized
}

fn publish_iteration_summary(
    output: Option<&OutputHandle>,
    iteration: usize,
    completed: bool,
    can_retry: bool,
    feedback: &str,
    max_iterations: usize,
) {
    let Some(output) = output else {
        return;
    };

    let summary = if completed {
        format!("Iteration {iteration}: all completion gates passed.")
    } else {
        format!("Iteration {iteration}: completion gates did not pass.")
    };
    let next_step = if completed {
        "Goal complete; no further iterations.".to_string()
    } else if can_retry {
        let reason = first_feedback_line(feedback).unwrap_or("completion gates failed");
        format!("Starting another iteration because {reason}.")
    } else if feedback.trim().is_empty() {
        format!("Stopping because max_iterations ({max_iterations}) was reached.")
    } else {
        let reason = first_feedback_line(feedback).unwrap_or("completion gates failed");
        format!(
            "Stopping because max_iterations ({max_iterations}) was reached; last reason: {reason}."
        )
    };

    output.iteration_summary(iteration, &summary, &next_step);
}

fn first_feedback_line(feedback: &str) -> Option<&str> {
    feedback
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
}

#[derive(Debug, Serialize)]
struct GoalEvent<'a> {
    event: &'a str,
    message: &'a str,
}

impl<'a> GoalEvent<'a> {
    fn new(event: &'a str, message: &'a str) -> Self {
        Self { event, message }
    }
}

#[derive(Debug, Serialize)]
struct AgentInputEvent<'a> {
    event: &'a str,
    iteration: usize,
    agent_id: &'a str,
    kind: String,
    output_contract: &'a str,
    input: &'a AgentInput,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::agent::{AgentSpec, NodeKind};
    use crate::config::default_orchestration_config;

    use super::build_context;

    #[test]
    fn default_goal_config_uses_contracts_not_roles() {
        let config = default_orchestration_config()
            .into_agent_config()
            .expect("default config is valid");

        assert!(config.specs.iter().all(|spec| spec.command == "copilot"));
        assert!(
            config
                .specs
                .iter()
                .any(|spec| spec.output_contract == "coverage")
        );
        assert!(
            config
                .specs
                .iter()
                .any(|spec| spec.output_contract == "feedback")
        );
        assert_eq!(config.max_parallel_agents, Some(8));
    }

    #[test]
    fn context_uses_only_ordered_inputs_not_dependencies() {
        let spec = AgentSpec {
            id: "consumer".to_string(),
            kind: NodeKind::Agent,
            output_contract: "generic".to_string(),
            artifact_dir: None,
            phase_label: None,
            instruction: Some("planning.md".to_string()),
            command: "echo".to_string(),
            args: Vec::new(),
            input_sources: vec!["far-source".to_string()],
            depends_on: vec!["scheduler-only".to_string()],
            resources: Vec::new(),
            timeout_secs: None,
            retries: 0,
            retry_delay_ms: None,
        };
        let mut outputs = BTreeMap::new();
        outputs.insert(
            "far-source".to_string(),
            crate::agent::AgentOutput {
                agent_id: "far-source".to_string(),
                kind: NodeKind::Agent,
                output_contract: "generic".to_string(),
                phase_label: None,
                artifact_dir: None,
                content: "far content".to_string(),
                artifact_path: String::new(),
            },
        );
        outputs.insert(
            "scheduler-only".to_string(),
            crate::agent::AgentOutput {
                agent_id: "scheduler-only".to_string(),
                kind: NodeKind::Agent,
                output_contract: "generic".to_string(),
                phase_label: None,
                artifact_dir: None,
                content: "must not appear".to_string(),
                artifact_path: String::new(),
            },
        );

        let context = build_context(&spec, "goal", "", "", &outputs);

        assert!(context.contains("## far-source"));
        assert!(context.contains("far content"));
        assert!(!context.contains("must not appear"));
    }

    #[test]
    fn context_renders_multiple_inputs_in_configured_order() {
        let spec = AgentSpec {
            id: "consumer".to_string(),
            kind: NodeKind::Agent,
            output_contract: "generic".to_string(),
            artifact_dir: None,
            phase_label: None,
            instruction: Some("planning.md".to_string()),
            command: "echo".to_string(),
            args: Vec::new(),
            input_sources: vec![
                "second".to_string(),
                "goal".to_string(),
                "first".to_string(),
            ],
            depends_on: Vec::new(),
            resources: Vec::new(),
            timeout_secs: None,
            retries: 0,
            retry_delay_ms: None,
        };
        let mut outputs = BTreeMap::new();
        outputs.insert(
            "first".to_string(),
            crate::agent::AgentOutput {
                agent_id: "first".to_string(),
                kind: NodeKind::Agent,
                output_contract: "generic".to_string(),
                phase_label: None,
                artifact_dir: None,
                content: "first body".to_string(),
                artifact_path: String::new(),
            },
        );
        outputs.insert(
            "second".to_string(),
            crate::agent::AgentOutput {
                agent_id: "second".to_string(),
                kind: NodeKind::Agent,
                output_contract: "generic".to_string(),
                phase_label: None,
                artifact_dir: None,
                content: "second body".to_string(),
                artifact_path: String::new(),
            },
        );

        let context = build_context(&spec, "goal body", "", "", &outputs);

        assert_eq!(
            context,
            "## second\n\nsecond body\n\n## goal\n\ngoal body\n\n## first\n\nfirst body"
        );
    }
}

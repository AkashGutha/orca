use std::collections::BTreeMap;
use std::time::Duration;

use async_trait::async_trait;

use crate::agent::{AgentInput, AgentOutput, AgentSpec};
use crate::errors::AppError;
use crate::executor::CommandExecutor;
use crate::model::CommandSpec;
use crate::output::OutputHandle;

#[async_trait]
pub trait Agent: Send + Sync {
    async fn run(
        &self,
        spec: &AgentSpec,
        input: AgentInput,
        instruction: &str,
    ) -> Result<AgentOutput, AppError>;
}

#[derive(Debug, Clone, Default)]
pub struct ExternalCommandAgent {
    executor: CommandExecutor,
    output: Option<OutputHandle>,
}

impl ExternalCommandAgent {
    pub fn new(output: Option<OutputHandle>) -> Self {
        Self {
            executor: CommandExecutor,
            output,
        }
    }
}

#[async_trait]
impl Agent for ExternalCommandAgent {
    async fn run(
        &self,
        spec: &AgentSpec,
        input: AgentInput,
        instruction: &str,
    ) -> Result<AgentOutput, AppError> {
        let mut env = BTreeMap::new();
        env.insert("ORCA_AGENT_ID".to_string(), spec.id.clone());
        env.insert(
            "ORCA_OUTPUT_CONTRACT".to_string(),
            spec.output_contract.clone(),
        );
        env.insert("ORCA_NODE_KIND".to_string(), format!("{:?}", spec.kind));
        env.insert("ORCA_GOAL".to_string(), input.goal.clone());
        env.insert("ORCA_CONTEXT".to_string(), input.context.clone());
        env.insert("ORCA_INSTRUCTIONS".to_string(), instruction.to_string());

        let args = if spec.args.is_empty() && spec.command == "copilot" {
            default_copilot_args(&spec.output_contract, &input, instruction)
        } else {
            spec.args
                .iter()
                .map(|arg| render_arg(arg, &input, instruction))
                .collect()
        };

        let output_model = model_from_args(&spec.command, &args);
        let command = CommandSpec {
            id: spec.id.clone(),
            program: spec.command.clone(),
            args,
            depends_on: Vec::new(),
            env,
            cwd: None,
            timeout: spec.timeout_secs.map(Duration::from_secs),
            timeout_secs: spec.timeout_secs,
            retries: spec.retries,
            retry_delay_ms: spec.retry_delay_ms,
            output: self.output.clone(),
            output_label: Some(spec.output_contract.clone()),
            output_phase: spec.phase_label.clone(),
            output_model,
        };

        let result = self.executor.execute(command).await;
        if !result.succeeded() {
            let mut message = result
                .error
                .unwrap_or_else(|| "agent command failed".to_string());
            if !result.stderr.trim().is_empty() {
                message.push_str(": ");
                message.push_str(result.stderr.trim());
            }
            return Err(AppError::AgentFailed {
                agent_id: spec.id.clone(),
                message,
            });
        }

        Ok(AgentOutput {
            agent_id: spec.id.clone(),
            kind: spec.kind,
            output_contract: spec.output_contract.clone(),
            phase_label: spec.phase_label.clone(),
            artifact_dir: spec.artifact_dir.clone(),
            content: result.stdout,
            artifact_path: String::new(),
        })
    }
}

fn render_arg(template: &str, input: &AgentInput, instruction: &str) -> String {
    let prompt = build_prompt(input, instruction);
    template
        .replace("{goal}", &input.goal)
        .replace("{context}", &input.context)
        .replace("{instructions}", instruction)
        .replace("{prompt}", &prompt)
}

fn build_prompt(input: &AgentInput, instruction: &str) -> String {
    format!(
        "{instruction}\n\nGoal:\n{goal}\n\nContext:\n{context}",
        goal = input.goal,
        context = input.context
    )
}

pub(crate) fn default_copilot_args(
    output_contract: &str,
    input: &AgentInput,
    instruction: &str,
) -> Vec<String> {
    let mut args = vec!["--silent".to_string(), "--no-ask-user".to_string()];

    if output_contract == "implementation" {
        args.push("--model".to_string());
        args.push("gpt-5.5".to_string());
    }

    if matches!(output_contract, "implementation" | "test" | "parallel") {
        args.push("--allow-all-tools".to_string());
    }

    args.push("-p".to_string());
    args.push(build_prompt(input, instruction));
    args
}

fn model_from_args(command: &str, args: &[String]) -> Option<String> {
    if command != "copilot" {
        return None;
    }

    args.windows(2)
        .find(|pair| pair[0] == "--model")
        .map(|pair| pair[1].clone())
        .or_else(|| Some("default".to_string()))
}

#[cfg(test)]
mod tests {
    use crate::agent::AgentInput;

    use super::default_copilot_args;

    #[test]
    fn default_copilot_args_use_prompt_flag() {
        let args = default_copilot_args(
            "plan",
            &AgentInput {
                goal: "improve instructions".to_string(),
                context: "prior feedback".to_string(),
            },
            "plan carefully",
        );

        assert!(args.contains(&"-p".to_string()));
        assert!(args.contains(&"--silent".to_string()));
        assert!(args.contains(&"--no-ask-user".to_string()));
        assert!(!args.contains(&"--allow-all-tools".to_string()));
        assert!(args.iter().any(|arg| arg.contains("improve instructions")));
    }

    #[test]
    fn default_work_agent_args_allow_tools() {
        let args = default_copilot_args(
            "implementation",
            &AgentInput {
                goal: "implement".to_string(),
                context: String::new(),
            },
            "do the work",
        );

        assert!(args.contains(&"--allow-all-tools".to_string()));
        assert!(
            args.windows(2)
                .any(|pair| pair[0] == "--model" && pair[1] == "gpt-5.5")
        );
        assert!(args.contains(&"-p".to_string()));
    }

    #[test]
    fn default_feature_generation_agent_args_do_not_allow_tools() {
        let args = default_copilot_args(
            "feature",
            &AgentInput {
                goal: "clarify feature".to_string(),
                context: "prior feedback".to_string(),
            },
            "generate feature context",
        );

        assert!(args.contains(&"-p".to_string()));
        assert!(!args.contains(&"--allow-all-tools".to_string()));
    }
}

use std::path::PathBuf;

use serde::Serialize;

use crate::settings::Settings;

#[derive(Debug, Clone)]
pub struct GoalRequest {
    pub goal: String,
    pub config: Option<PathBuf>,
    pub settings: Settings,
    pub instruction_dir: Option<PathBuf>,
    pub artifact_dir: Option<PathBuf>,
    pub max_parallel_agents: Option<usize>,
    pub approve_golden_plan: bool,
    pub json: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct GoalSummary {
    pub goal: String,
    pub artifact_root: String,
    pub approved: bool,
    pub completed: bool,
    pub iterations: usize,
    pub feedback: Option<String>,
    pub golden_plan_path: Option<String>,
    pub outputs: Vec<crate::agent::AgentOutput>,
    pub json: bool,
}

impl GoalSummary {
    pub fn exit_code(&self) -> u8 {
        if self.completed { 0 } else { 1 }
    }
}

impl std::fmt::Display for GoalSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "goal: {}", self.goal)?;
        writeln!(f, "artifact_root: {}", self.artifact_root)?;
        writeln!(f, "approved: {}", self.approved)?;
        writeln!(f, "completed: {}", self.completed)?;
        writeln!(f, "iterations: {}", self.iterations)?;
        if let Some(feedback) = &self.feedback {
            writeln!(f, "feedback: {feedback}")?;
        }
        if let Some(path) = &self.golden_plan_path {
            writeln!(f, "golden_plan: {path}")?;
        }
        for output in &self.outputs {
            writeln!(
                f,
                "{} {}: {}",
                output.agent_id, output.output_contract, output.artifact_path
            )?;
        }
        write!(f, "exit_code={}", self.exit_code())
    }
}

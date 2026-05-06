use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct AgentConfig {
    pub instruction_dir: Option<PathBuf>,
    pub artifact_dir: Option<PathBuf>,
    pub max_parallel_agents: Option<usize>,
    pub min_successful_planners: Option<usize>,
    pub min_successful_critics: Option<usize>,
    pub max_iterations: Option<usize>,
    pub approval_mode: Option<ApprovalMode>,
    #[serde(default)]
    pub specs: Vec<AgentSpec>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalMode {
    Manual,
    Auto,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentSpec {
    pub id: String,
    pub kind: NodeKind,
    pub output_contract: String,
    pub artifact_dir: Option<String>,
    pub phase_label: Option<String>,
    pub instruction: Option<String>,
    #[serde(default = "default_agent_command")]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub input_sources: Vec<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub resources: Vec<String>,
    pub timeout_secs: Option<u64>,
    #[serde(default)]
    pub retries: u32,
    pub retry_delay_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Agent,
    Branch,
    Gate,
    Aggregate,
    Approval,
    Feedback,
    LoopFeedback,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentInput {
    pub goal: String,
    pub context: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentOutput {
    pub agent_id: String,
    pub kind: NodeKind,
    pub output_contract: String,
    pub phase_label: Option<String>,
    pub artifact_dir: Option<String>,
    pub content: String,
    pub artifact_path: String,
}

fn default_agent_command() -> String {
    "copilot".to_string()
}

#[cfg(test)]
mod tests {
    use super::NodeKind;

    #[test]
    fn node_kind_uses_snake_case_serde_name() {
        let serialized = serde_json::to_string(&NodeKind::LoopFeedback).unwrap();
        assert_eq!(serialized, "\"loop_feedback\"");

        let deserialized: NodeKind = serde_json::from_str("\"loop_feedback\"").unwrap();
        assert_eq!(deserialized, NodeKind::LoopFeedback);
    }
}

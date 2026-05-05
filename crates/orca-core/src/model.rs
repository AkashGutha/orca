use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use serde::Serialize;

use crate::output::OutputHandle;

#[derive(Debug, Clone, Serialize)]
pub struct ExecutionPlan {
    pub max_concurrency: usize,
    pub continue_on_error: bool,
    pub commands: Vec<CommandSpec>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommandSpec {
    pub id: String,
    pub program: String,
    pub args: Vec<String>,
    pub depends_on: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub cwd: Option<PathBuf>,
    #[serde(skip)]
    pub timeout: Option<Duration>,
    pub timeout_secs: Option<u64>,
    pub retries: u32,
    pub retry_delay_ms: Option<u64>,
    #[serde(skip)]
    pub output: Option<OutputHandle>,
    #[serde(skip)]
    pub output_label: Option<String>,
    #[serde(skip)]
    pub output_phase: Option<String>,
    #[serde(skip)]
    pub output_model: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommandOutcome {
    Success,
    Failed,
    TimedOut,
    SpawnFailed,
    Cancelled,
    Skipped,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommandResult {
    pub id: String,
    pub outcome: CommandOutcome,
    pub exit_code: Option<i32>,
    pub attempts: u32,
    pub stdout: String,
    pub stderr: String,
    pub error: Option<String>,
}

impl CommandResult {
    pub fn succeeded(&self) -> bool {
        self.outcome == CommandOutcome::Success
    }

    pub fn failed(&self) -> bool {
        !self.succeeded()
    }

    pub fn skipped(id: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            outcome: CommandOutcome::Skipped,
            exit_code: None,
            attempts: 0,
            stdout: String::new(),
            stderr: String::new(),
            error: Some(reason.into()),
        }
    }

    pub fn cancelled(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            outcome: CommandOutcome::Cancelled,
            exit_code: None,
            attempts: 0,
            stdout: String::new(),
            stderr: String::new(),
            error: Some("cancelled after earlier failure".to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RunSummary {
    pub results: Vec<CommandResult>,
}

impl RunSummary {
    pub fn exit_code(&self) -> u8 {
        if self.results.iter().any(CommandResult::failed) {
            1
        } else {
            0
        }
    }
}

impl fmt::Display for RunSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for result in &self.results {
            writeln!(
                f,
                "{}: {:?} attempts={} exit_code={:?}",
                result.id, result.outcome, result.attempts, result.exit_code
            )?;
            if let Some(error) = &result.error {
                writeln!(f, "  error: {error}")?;
            }
        }
        write!(f, "exit_code={}", self.exit_code())
    }
}

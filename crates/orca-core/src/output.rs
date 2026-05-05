use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use serde::Serialize;
use tokio::sync::mpsc;

pub mod state;

pub use state::{AgentPaneState, IterationSummaryState, OutputState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum OutputStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum AgentStatus {
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
pub enum OutputEvent {
    PhaseStarted {
        phase: String,
    },
    AgentStarted {
        id: String,
        label: String,
        phase: String,
        model: Option<String>,
    },
    AgentInput {
        id: String,
        glimpse: String,
    },
    Line {
        id: String,
        stream: OutputStream,
        line: String,
    },
    AgentFinished {
        id: String,
        status: AgentStatus,
    },
    IterationSummary {
        iteration: usize,
        summary: String,
        next_step: String,
    },
    Shutdown,
}

pub type OutputObserver = Arc<dyn Fn(OutputEvent) + Send + Sync + 'static>;

#[derive(Clone)]
pub struct OutputHandle {
    sender: Option<mpsc::UnboundedSender<OutputEvent>>,
    stop_requested: Arc<AtomicBool>,
    observer: Option<OutputObserver>,
}

impl std::fmt::Debug for OutputHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OutputHandle")
            .field("has_sender", &self.sender.is_some())
            .field("stop_requested", &self.stop_requested())
            .field("has_observer", &self.observer.is_some())
            .finish_non_exhaustive()
    }
}

impl OutputHandle {
    pub fn new(
        sender: mpsc::UnboundedSender<OutputEvent>,
        stop_requested: Arc<AtomicBool>,
        observer: Option<OutputObserver>,
    ) -> Self {
        Self {
            sender: Some(sender),
            stop_requested,
            observer,
        }
    }

    pub fn observer(observer: OutputObserver) -> Self {
        Self {
            sender: None,
            stop_requested: Arc::new(AtomicBool::new(false)),
            observer: Some(observer),
        }
    }

    pub fn phase_started(&self, phase: &str) {
        self.emit(OutputEvent::PhaseStarted {
            phase: phase.to_string(),
        });
    }

    pub fn agent_started(&self, id: &str, label: &str, phase: &str, model: Option<&str>) {
        self.emit(OutputEvent::AgentStarted {
            id: id.to_string(),
            label: label.to_string(),
            phase: phase.to_string(),
            model: model.map(str::to_string),
        });
    }

    pub fn agent_input(&self, id: &str, glimpse: &str) {
        self.emit(OutputEvent::AgentInput {
            id: id.to_string(),
            glimpse: glimpse.to_string(),
        });
    }

    pub fn line(&self, id: &str, stream: OutputStream, line: &str) {
        self.emit(OutputEvent::Line {
            id: id.to_string(),
            stream,
            line: line.to_string(),
        });
    }

    pub fn agent_finished(&self, id: &str, status: AgentStatus) {
        self.emit(OutputEvent::AgentFinished {
            id: id.to_string(),
            status,
        });
    }

    pub fn iteration_summary(&self, iteration: usize, summary: &str, next_step: &str) {
        self.emit(OutputEvent::IterationSummary {
            iteration,
            summary: summary.to_string(),
            next_step: next_step.to_string(),
        });
    }

    pub fn shutdown(&self) {
        self.emit(OutputEvent::Shutdown);
    }

    pub fn request_stop(&self) {
        self.stop_requested.store(true, Ordering::SeqCst);
    }

    pub fn stop_requested(&self) -> bool {
        self.stop_requested.load(Ordering::SeqCst)
    }

    fn emit(&self, event: OutputEvent) {
        if let Some(observer) = &self.observer {
            observer(event.clone());
        }
        if let Some(sender) = &self.sender {
            let _ = sender.send(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AgentStatus, OutputEvent, OutputHandle, OutputState, OutputStream};

    #[test]
    fn output_state_tracks_agent_lines_and_status() {
        let mut state = OutputState::default();

        assert!(state.apply(OutputEvent::AgentStarted {
            id: "agent-a".to_string(),
            label: "plan".to_string(),
            phase: "planning".to_string(),
            model: Some("default".to_string()),
        }));
        assert!(state.apply(OutputEvent::Line {
            id: "agent-a".to_string(),
            stream: OutputStream::Stdout,
            line: "hello".to_string(),
        }));
        assert!(state.apply(OutputEvent::AgentFinished {
            id: "agent-a".to_string(),
            status: AgentStatus::Succeeded,
        }));

        let pane = state.panes().next().unwrap();
        assert_eq!(pane.status, AgentStatus::Succeeded);
        assert_eq!(pane.lines.back().unwrap(), "hello");
    }

    #[test]
    fn phase_started_clears_old_panes() {
        let mut state = OutputState::default();

        assert!(state.apply(OutputEvent::AgentStarted {
            id: "planner-a".to_string(),
            label: "plan".to_string(),
            phase: "planning".to_string(),
            model: Some("default".to_string()),
        }));
        assert_eq!(state.panes().count(), 1);

        assert!(state.apply(OutputEvent::PhaseStarted {
            phase: "work".to_string(),
        }));

        assert_eq!(state.panes().count(), 0);
        assert_eq!(state.active_phase(), Some("work"));
    }

    #[test]
    fn output_state_removes_control_characters_without_prefixing_lines() {
        let mut state = OutputState::default();

        assert!(state.apply(OutputEvent::Line {
            id: "agent-a".to_string(),
            stream: OutputStream::Stderr,
            line: "\u{1b}[31mthinking\r".to_string(),
        }));

        let pane = state.panes().next().unwrap();
        assert_eq!(pane.lines.back().unwrap(), "thinking");
    }

    #[test]
    fn output_state_shows_agent_input_glimpse() {
        let mut state = OutputState::default();

        assert!(state.apply(OutputEvent::AgentStarted {
            id: "planner-a".to_string(),
            label: "plan".to_string(),
            phase: "planning".to_string(),
            model: Some("default".to_string()),
        }));
        assert!(state.apply(OutputEvent::AgentInput {
            id: "planner-a".to_string(),
            glimpse: "goal: ship it | context: feedback".to_string(),
        }));

        let pane = state.panes().next().unwrap();
        assert_eq!(
            pane.lines.back().unwrap(),
            "Input: goal: ship it | context: feedback"
        );
    }

    #[test]
    fn output_handle_records_stop_requests() {
        let (sender, _receiver) = tokio::sync::mpsc::unbounded_channel();
        let handle = OutputHandle::new(
            sender,
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            None,
        );

        assert!(!handle.stop_requested());
        handle.request_stop();
        assert!(handle.stop_requested());
    }

    #[test]
    fn iteration_summary_clears_agent_panes() {
        let mut state = OutputState::default();

        assert!(state.apply(OutputEvent::AgentStarted {
            id: "agent-a".to_string(),
            label: "implementation".to_string(),
            phase: "work".to_string(),
            model: Some("gpt-5.5".to_string()),
        }));
        assert_eq!(state.panes().count(), 1);

        assert!(state.apply(OutputEvent::IterationSummary {
            iteration: 1,
            summary: "tests failed".to_string(),
            next_step: "retrying because coverage is missing".to_string(),
        }));

        assert_eq!(state.panes().count(), 0);
        assert_eq!(state.iteration_summary().unwrap().iteration, 1);
    }
}

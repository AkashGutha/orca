use std::collections::{BTreeMap, VecDeque};

use crate::output::{AgentStatus, OutputEvent, OutputStream};

const MAX_LINES_PER_AGENT: usize = 400;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentPaneState {
    pub id: String,
    pub label: String,
    pub phase: String,
    pub model: Option<String>,
    pub status: AgentStatus,
    pub lines: VecDeque<String>,
}

impl AgentPaneState {
    fn new(id: String, label: String, phase: String, model: Option<String>) -> Self {
        Self {
            id,
            label,
            phase,
            model,
            status: AgentStatus::Running,
            lines: VecDeque::new(),
        }
    }

    fn push_line(&mut self, _stream: OutputStream, line: String) {
        if self.lines.len() >= MAX_LINES_PER_AGENT {
            self.lines.pop_front();
        }
        self.lines.push_back(sanitize_line(&line));
    }
}

#[derive(Debug, Default)]
pub struct OutputState {
    panes: BTreeMap<String, AgentPaneState>,
    iteration_summary: Option<IterationSummaryState>,
    active_phase: Option<String>,
    stop_requested: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IterationSummaryState {
    pub iteration: usize,
    pub summary: String,
    pub next_step: String,
}

impl OutputState {
    pub fn apply(&mut self, event: OutputEvent) -> bool {
        match event {
            OutputEvent::PhaseStarted { phase } => {
                self.panes.clear();
                self.iteration_summary = None;
                self.active_phase = Some(phase);
                true
            }
            OutputEvent::AgentStarted {
                id,
                label,
                phase,
                model,
            } => {
                self.iteration_summary = None;
                self.active_phase = Some(phase.clone());
                self.panes
                    .insert(id.clone(), AgentPaneState::new(id, label, phase, model));
                true
            }
            OutputEvent::Line { id, stream, line } => {
                let pane = self
                    .panes
                    .entry(id.clone())
                    .or_insert_with(|| AgentPaneState::new(id, String::new(), String::new(), None));
                pane.push_line(stream, line);
                true
            }
            OutputEvent::AgentInput { id, glimpse } => {
                let pane = self
                    .panes
                    .entry(id.clone())
                    .or_insert_with(|| AgentPaneState::new(id, String::new(), String::new(), None));
                pane.push_line(OutputStream::Stdout, format!("Input: {glimpse}"));
                true
            }
            OutputEvent::AgentFinished { id, status } => {
                let pane = self
                    .panes
                    .entry(id.clone())
                    .or_insert_with(|| AgentPaneState::new(id, String::new(), String::new(), None));
                pane.status = status;
                true
            }
            OutputEvent::IterationSummary {
                iteration,
                summary,
                next_step,
            } => {
                self.panes.clear();
                self.active_phase = None;
                self.iteration_summary = Some(IterationSummaryState {
                    iteration,
                    summary: sanitize_line(&summary),
                    next_step: sanitize_line(&next_step),
                });
                true
            }
            OutputEvent::Shutdown => false,
        }
    }

    pub fn panes(&self) -> impl Iterator<Item = &AgentPaneState> {
        self.panes.values()
    }

    pub fn active_phase(&self) -> Option<&str> {
        self.active_phase.as_deref()
    }

    pub fn iteration_summary(&self) -> Option<&IterationSummaryState> {
        self.iteration_summary.as_ref()
    }

    pub fn stop_requested(&self) -> bool {
        self.stop_requested
    }

    pub fn request_stop(&mut self) {
        self.stop_requested = true;
    }
}

pub fn sanitize_line(line: &str) -> String {
    let mut sanitized = String::new();
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

        match ch {
            '\t' => sanitized.push(' '),
            ch if ch.is_control() => {}
            ch => sanitized.push(ch),
        }
    }

    sanitized
}

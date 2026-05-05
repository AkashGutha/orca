use tokio::sync::mpsc;

use orca_core::output::OutputEvent;
use orca_core::output::state::sanitize_line;

pub async fn run(mut receiver: mpsc::UnboundedReceiver<OutputEvent>) {
    while let Some(event) = receiver.recv().await {
        match event {
            OutputEvent::PhaseStarted { phase } => {
                println!("\n== {phase} phase ==");
            }
            OutputEvent::AgentStarted {
                id,
                label,
                phase,
                model,
            } => {
                let model = model.unwrap_or_else(|| "n/a".to_string());
                println!("\n== {id} | {label} | {phase} | model: {model} ==");
            }
            OutputEvent::Line { line, .. } => {
                println!("{}", sanitize_line(&line));
            }
            OutputEvent::AgentInput { id, glimpse } => {
                println!("{id} input: {}", sanitize_line(&glimpse));
            }
            OutputEvent::AgentFinished { id, status } => {
                println!("== {id} | {status:?} ==");
            }
            OutputEvent::IterationSummary {
                iteration,
                summary,
                next_step,
            } => {
                println!("\n== iteration {iteration} summary ==");
                println!("{}", sanitize_line(&summary));
                println!("{}", sanitize_line(&next_step));
            }
            OutputEvent::Shutdown => break,
        }
    }
}

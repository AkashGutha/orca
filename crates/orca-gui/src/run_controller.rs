use std::sync::{Arc, Mutex};

use eframe::egui;
use orca_core::goal::{GoalRequest, GoalSummary};
use orca_core::orchestrator::GoalOrchestrator;
use orca_core::output::{OutputEvent, OutputHandle, OutputObserver, OutputState};

#[derive(Default)]
pub(super) struct RunState {
    pub output: OutputState,
    pub running: bool,
    pub summary: Option<GoalSummary>,
    pub error: Option<String>,
}

pub(super) fn start_run(request: GoalRequest, run_state: Arc<Mutex<RunState>>, ctx: egui::Context) {
    if let Ok(mut state) = run_state.lock() {
        *state = RunState {
            running: true,
            ..RunState::default()
        };
    }

    std::thread::spawn(move || {
        let observer_state = Arc::clone(&run_state);
        let observer_ctx = ctx.clone();
        let observer: OutputObserver = Arc::new(move |event: OutputEvent| {
            if let Ok(mut state) = observer_state.lock() {
                state.output.apply(event);
            }
            observer_ctx.request_repaint();
        });

        let result = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|err| err.to_string())
            .and_then(|runtime| {
                let output = OutputHandle::observer(observer);
                runtime
                    .block_on(GoalOrchestrator.run_with_output_handle(request, Some(output)))
                    .map_err(|err| err.to_string())
            });

        if let Ok(mut state) = run_state.lock() {
            state.running = false;
            match result {
                Ok(summary) => state.summary = Some(summary),
                Err(error) => state.error = Some(error),
            }
        }
        ctx.request_repaint();
    });
}

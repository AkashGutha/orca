use eframe::egui;
use orca_core::output::AgentStatus;

use crate::run_controller::RunState;
use crate::ui_config::GuiConfig;

pub(super) fn draw_output(ui: &mut egui::Ui, state: &RunState, config: &GuiConfig) {
    if let Some(error) = &state.error {
        ui.colored_label(egui::Color32::RED, error);
    }
    if let Some(summary) = &state.summary {
        ui.label(format!(
            "Completed: {} | approved: {} | iterations: {}",
            summary.completed, summary.approved, summary.iterations
        ));
    }

    if let Some(summary) = state.output.iteration_summary() {
        ui.group(|ui| {
            ui.heading(format!("Iteration {} summary", summary.iteration));
            ui.label(&summary.summary);
            ui.label(&summary.next_step);
        });
        return;
    }

    let panes = state
        .output
        .panes()
        .take(config.max_visible_panes)
        .collect::<Vec<_>>();
    if panes.is_empty() {
        let message = state
            .output
            .active_phase()
            .map(|phase| format!("Starting {phase} phase..."))
            .unwrap_or_else(|| "Waiting for agents...".to_string());
        ui.label(message);
        return;
    }

    ui.columns(panes.len(), |columns| {
        for (pane, column) in panes.into_iter().zip(columns.iter_mut()) {
            column.group(|ui| {
                ui.set_width(ui.available_width());
                let color = match pane.status {
                    AgentStatus::Running => egui::Color32::LIGHT_BLUE,
                    AgentStatus::Succeeded => egui::Color32::GREEN,
                    AgentStatus::Failed => egui::Color32::RED,
                };
                ui.colored_label(
                    color,
                    format!(
                        "{} | {} | {}",
                        pane.id,
                        pane.label,
                        pane.model.as_deref().unwrap_or("n/a")
                    ),
                );
                ui.separator();
                let text_width = ui.available_width();
                egui::ScrollArea::vertical()
                    .id_salt(format!("agent-log-{}", pane.id))
                    .stick_to_bottom(true)
                    .max_height(ui.available_height().max(240.0))
                    .show(ui, |ui| {
                        ui.set_width(text_width);
                        for line in &pane.lines {
                            ui.add(egui::Label::new(egui::RichText::new(line).monospace()).wrap());
                        }
                    });
            });
        }
    });
}

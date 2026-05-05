use std::path::PathBuf;

use eframe::egui;
use orca_core::config::DEFAULT_CONFIG_PATH;
use orca_core::settings::Settings;

use crate::app::ControlFields;

pub(super) fn draw_controls(
    ui: &mut egui::Ui,
    fields: &mut ControlFields,
    settings: &Settings,
    running: bool,
) -> bool {
    ui.heading("Goal");
    ui.text_edit_multiline(&mut fields.goal);
    ui.separator();
    config_picker(ui, &mut fields.config, settings);
    labeled_text(ui, "Artifacts", &mut fields.artifact_dir, "optional");
    labeled_text(ui, "Instructions", &mut fields.instruction_dir, "optional");
    labeled_text(
        ui,
        "Max parallel agents",
        &mut fields.max_parallel_agents,
        "optional",
    );
    ui.checkbox(&mut fields.approve_golden_plan, "Approve golden plan");

    ui.add_enabled(!running, egui::Button::new("Run goal"))
        .clicked()
}

fn labeled_text(ui: &mut egui::Ui, label: &str, value: &mut String, hint: &str) {
    ui.label(label);
    ui.add(egui::TextEdit::singleline(value).hint_text(hint));
}

fn config_picker(ui: &mut egui::Ui, value: &mut String, settings: &Settings) {
    ui.label("Config");
    let workflow_configs = settings.workflow_configs();
    if workflow_configs.is_empty() {
        ui.small("No workflow configs found in settings workflow directories.");
    } else {
        egui::ComboBox::from_id_salt("workflow-config-picker")
            .selected_text(selected_config_label(value))
            .show_ui(ui, |ui| {
                for config in workflow_configs {
                    let selected = PathBuf::from(value.trim()) == config;
                    if ui
                        .selectable_label(selected, workflow_config_label(&config))
                        .on_hover_text(config.display().to_string())
                        .clicked()
                    {
                        *value = config.display().to_string();
                    }
                }
            });
    }
    ui.horizontal_wrapped(|ui| {
        if ui.button("Browse...").clicked()
            && let Some((directory, file_name)) = config_picker_start(value)
            && let Some(path) = rfd::FileDialog::new()
                .set_title("Load ORCA configuration")
                .set_directory(directory)
                .set_file_name(file_name)
                .add_filter("ORCA config", &["toml", "yaml", "yml"])
                .add_filter("TOML", &["toml"])
                .add_filter("YAML", &["yaml", "yml"])
                .pick_file()
        {
            *value = path.display().to_string();
        }
        if ui.button("Use default").clicked() {
            *value = settings.resolve_workflow_config(None).display().to_string();
        }
    });
    let selected = selected_config_label(value);
    ui.small(selected)
        .on_hover_text(if value.trim().is_empty() {
            DEFAULT_CONFIG_PATH.to_string()
        } else {
            value.clone()
        });
}

fn workflow_config_label(path: &std::path::Path) -> String {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_else(|| path.to_str().unwrap_or("config"));
    let parent = path
        .parent()
        .and_then(|parent| parent.to_str())
        .filter(|parent| !parent.is_empty());
    match parent {
        Some(parent) => format!("{file_name} ({parent})"),
        None => file_name.to_string(),
    }
}

fn selected_config_label(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return format!("Selected: {DEFAULT_CONFIG_PATH}");
    }
    let path = PathBuf::from(trimmed);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(trimmed);
    format!("Selected: {file_name}")
}

fn config_picker_start(value: &str) -> Option<(PathBuf, String)> {
    let selected = PathBuf::from(value.trim());
    if selected.is_file() {
        let directory = selected
            .parent()
            .map(absolute_path)
            .unwrap_or_else(current_directory);
        let file_name = selected
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("orca.default.toml")
            .to_string();
        return Some((directory, file_name));
    }
    if selected.is_dir() {
        return Some((absolute_path(&selected), "orca.default.toml".to_string()));
    }

    let default_config = PathBuf::from(DEFAULT_CONFIG_PATH);
    if default_config.is_file()
        && let Some(parent) = default_config.parent()
    {
        return Some((absolute_path(parent), "orca.default.toml".to_string()));
    }

    let config_dir = PathBuf::from("config");
    if config_dir.is_dir() {
        return Some((absolute_path(&config_dir), "orca.default.toml".to_string()));
    }

    Some((current_directory(), "orca.default.toml".to_string()))
}

fn absolute_path(path: &std::path::Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            current_directory().join(path)
        }
    })
}

fn current_directory() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

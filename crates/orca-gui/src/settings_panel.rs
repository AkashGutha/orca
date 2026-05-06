use std::path::PathBuf;

use eframe::egui;
use orca_core::settings::Settings;

pub(super) enum SettingsPanelAction {
    None,
    Load,
    Save,
    Apply,
    Close,
}

pub(super) fn draw_settings_panel(
    ui: &mut egui::Ui,
    settings: &mut Settings,
    settings_path: &mut String,
    status: Option<&str>,
) -> SettingsPanelAction {
    let mut action = SettingsPanelAction::None;

    ui.horizontal(|ui| {
        ui.heading("Settings");
        if ui.button("Close").clicked() {
            action = SettingsPanelAction::Close;
        }
    });

    ui.separator();
    ui.label("Settings file");
    ui.horizontal_wrapped(|ui| {
        ui.add(egui::TextEdit::singleline(settings_path).hint_text("settings.toml"));
        if ui.button("Browse...").clicked()
            && let Some(path) = rfd::FileDialog::new()
                .set_title("Open ORCA settings")
                .add_filter("TOML", &["toml"])
                .pick_file()
        {
            *settings_path = path.display().to_string();
        }
    });
    ui.horizontal(|ui| {
        if ui.button("Load").clicked() {
            action = SettingsPanelAction::Load;
        }
        if ui.button("Save").clicked() {
            action = SettingsPanelAction::Save;
        }
        if ui.button("Apply to run controls").clicked() {
            action = SettingsPanelAction::Apply;
        }
    });
    if let Some(status) = status {
        ui.small(status);
    }

    ui.separator();
    source_directories(ui, "Agent source directories", &mut settings.sources.agents);
    ui.separator();
    source_directories(
        ui,
        "Instruction source directories",
        &mut settings.sources.instructions,
    );
    ui.separator();
    source_directories(ui, "Skill source directories", &mut settings.sources.skills);
    ui.separator();
    source_directories(
        ui,
        "Workflow config source directories",
        &mut settings.sources.workflows,
    );

    ui.separator();
    ui.heading("Defaults");
    optional_path_field(
        ui,
        "Default workflow",
        &mut settings.defaults.workflow,
        false,
    );
    optional_path_field(
        ui,
        "Default artifact directory",
        &mut settings.defaults.artifact_dir,
        true,
    );
    optional_usize_field(
        ui,
        "Default max parallel agents",
        &mut settings.defaults.max_parallel_agents,
    );

    action
}

fn source_directories(ui: &mut egui::Ui, heading: &str, sources: &mut Vec<PathBuf>) {
    ui.heading(heading);
    let mut remove_index = None;
    for (index, source) in sources.iter_mut().enumerate() {
        ui.horizontal_wrapped(|ui| {
            editable_path(ui, source);
            if ui.button("Browse...").clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .set_title(heading)
                    .set_directory(source.clone())
                    .pick_folder()
            {
                *source = path;
            }
            if ui.button("Remove").clicked() {
                remove_index = Some(index);
            }
        });
    }
    if let Some(index) = remove_index {
        sources.remove(index);
    }
    if ui.button("Add directory").clicked() {
        sources.push(PathBuf::new());
    }
}

fn optional_path_field(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Option<PathBuf>,
    directory: bool,
) {
    ui.label(label);
    ui.horizontal_wrapped(|ui| {
        let mut text = value
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_default();
        if ui
            .add(egui::TextEdit::singleline(&mut text).hint_text("optional"))
            .changed()
        {
            *value = path_from_text(&text);
        }
        if ui.button("Browse...").clicked() {
            let dialog = rfd::FileDialog::new().set_title(label);
            let path = if directory {
                dialog.pick_folder()
            } else {
                dialog
                    .add_filter("ORCA config", &["toml", "yaml", "yml"])
                    .pick_file()
            };
            if let Some(path) = path {
                *value = Some(path);
            }
        }
        if ui.button("Clear").clicked() {
            *value = None;
        }
    });
}

fn optional_usize_field(ui: &mut egui::Ui, label: &str, value: &mut Option<usize>) {
    ui.label(label);
    let mut text = value.map(|value| value.to_string()).unwrap_or_default();
    if ui
        .add(egui::TextEdit::singleline(&mut text).hint_text("optional"))
        .changed()
    {
        *value = if text.trim().is_empty() {
            None
        } else {
            text.trim().parse().ok()
        };
    }
}

fn editable_path(ui: &mut egui::Ui, path: &mut PathBuf) {
    let mut text = path.display().to_string();
    if ui.add(egui::TextEdit::singleline(&mut text)).changed() {
        *path = PathBuf::from(text.trim());
    }
}

fn path_from_text(text: &str) -> Option<PathBuf> {
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

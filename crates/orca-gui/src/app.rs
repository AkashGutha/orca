use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use eframe::egui;
use orca_core::config::DEFAULT_CONFIG_PATH;
use orca_core::goal::GoalRequest;
use orca_core::settings::{DEFAULT_SETTINGS_PATH, Settings};

use crate::controls;
use crate::run_controller::{self, RunState};
use crate::settings_panel::{self, SettingsPanelAction};
use crate::ui_config::GuiUiConfig;

pub struct OrcaApp {
    fields: ControlFields,
    run_state: Arc<Mutex<RunState>>,
    settings: Settings,
    settings_path: String,
    show_settings: bool,
    settings_status: Option<String>,
    ui_config: GuiUiConfig,
}

pub(super) struct ControlFields {
    pub goal: String,
    pub config: String,
    pub artifact_dir: String,
    pub instruction_dir: String,
    pub max_parallel_agents: String,
    pub approve_golden_plan: bool,
}

impl Default for OrcaApp {
    fn default() -> Self {
        let loaded_settings = Settings::load_default();
        let (settings, settings_path, settings_status) = match loaded_settings {
            Ok(loaded) => (
                loaded.settings,
                loaded
                    .path
                    .unwrap_or_else(|| PathBuf::from(DEFAULT_SETTINGS_PATH))
                    .display()
                    .to_string(),
                None,
            ),
            Err(error) => (
                Settings::default(),
                DEFAULT_SETTINGS_PATH.to_string(),
                Some(error.to_string()),
            ),
        };
        let fields = ControlFields::from_settings(&settings);
        Self {
            fields,
            run_state: Arc::new(Mutex::new(RunState::default())),
            settings,
            settings_path,
            show_settings: false,
            settings_status,
            ui_config: GuiUiConfig::default(),
        }
    }
}

impl eframe::App for OrcaApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("🐋 ORCA");
                let running = self.run_state.lock().is_ok_and(|state| state.running);
                if running {
                    ui.label("Running");
                }
                ui.allocate_ui_with_layout(
                    ui.available_size(),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        if ui.button("Settings").clicked() {
                            self.show_settings = !self.show_settings;
                        }
                    },
                );
            });
        });

        if self.show_settings {
            egui::SidePanel::right("settings")
                .resizable(true)
                .show(ctx, |ui| {
                    match settings_panel::draw_settings_panel(
                        ui,
                        &mut self.settings,
                        &mut self.settings_path,
                        self.settings_status.as_deref(),
                    ) {
                        SettingsPanelAction::None => {}
                        SettingsPanelAction::Load => self.load_settings_from_panel(),
                        SettingsPanelAction::Save => self.save_settings_from_panel(),
                        SettingsPanelAction::Apply => self.apply_settings_to_controls(),
                        SettingsPanelAction::Close => self.show_settings = false,
                    }
                });
        }

        egui::SidePanel::left("controls")
            .resizable(true)
            .show(ctx, |ui| {
                let running = self.run_state.lock().is_ok_and(|state| state.running);
                if controls::draw_controls(ui, &mut self.fields, &self.settings, running) {
                    self.start_run(ctx.clone());
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            let state = self.run_state.lock().expect("GUI run state mutex poisoned");
            crate::panes::draw_output(ui, &state, &self.ui_config.gui);
        });
    }
}

impl OrcaApp {
    fn start_run(&mut self, ctx: egui::Context) {
        let request = match self.request_from_fields() {
            Ok(request) => request,
            Err(error) => {
                if let Ok(mut state) = self.run_state.lock() {
                    state.error = Some(error);
                }
                return;
            }
        };

        run_controller::start_run(request, Arc::clone(&self.run_state), ctx);
    }

    fn request_from_fields(&self) -> Result<GoalRequest, String> {
        let goal = self.fields.goal.trim();
        if goal.is_empty() {
            return Err("Enter a goal before starting a run.".to_string());
        }

        Ok(GoalRequest {
            goal: goal.to_string(),
            config: optional_path(&self.fields.config),
            settings: self.settings.clone(),
            instruction_dir: optional_path(&self.fields.instruction_dir),
            artifact_dir: optional_path(&self.fields.artifact_dir),
            max_parallel_agents: parse_optional_usize(&self.fields.max_parallel_agents)?,
            approve_golden_plan: self.fields.approve_golden_plan,
            json: false,
        })
    }

    fn load_settings_from_panel(&mut self) {
        let path = optional_path(&self.settings_path);
        match Settings::load_optional(path.as_deref()) {
            Ok(loaded) => {
                self.settings = loaded.settings;
                if let Some(path) = loaded.path {
                    self.settings_path = path.display().to_string();
                } else if self.settings_path.trim().is_empty() {
                    self.settings_path = DEFAULT_SETTINGS_PATH.to_string();
                }
                self.settings_status = Some("Settings loaded".to_string());
                self.apply_settings_to_controls();
            }
            Err(error) => {
                self.settings_status = Some(error.to_string());
            }
        }
    }

    fn save_settings_from_panel(&mut self) {
        let path = optional_path(&self.settings_path)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_SETTINGS_PATH));
        match self.settings.save_to_path(&path) {
            Ok(()) => {
                self.settings_path = path.display().to_string();
                self.settings_status = Some("Settings saved".to_string());
                self.apply_settings_to_controls();
            }
            Err(error) => {
                self.settings_status = Some(error.to_string());
            }
        }
    }

    fn apply_settings_to_controls(&mut self) {
        self.fields.apply_settings(&self.settings);
    }
}

impl ControlFields {
    fn from_settings(settings: &Settings) -> Self {
        let mut fields = Self {
            goal: String::new(),
            config: DEFAULT_CONFIG_PATH.to_string(),
            artifact_dir: String::new(),
            instruction_dir: String::new(),
            max_parallel_agents: String::new(),
            approve_golden_plan: false,
        };
        fields.apply_settings(settings);
        fields
    }

    fn apply_settings(&mut self, settings: &Settings) {
        self.config = settings.resolve_workflow_config(None).display().to_string();
        self.artifact_dir = settings
            .default_artifact_dir()
            .map(|path| path.display().to_string())
            .unwrap_or_default();
        self.max_parallel_agents = settings
            .default_max_parallel_agents()
            .map(|value| value.to_string())
            .unwrap_or_default();
    }
}

fn optional_path(value: &str) -> Option<PathBuf> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

fn parse_optional_usize(value: &str) -> Result<Option<usize>, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    trimmed
        .parse()
        .map(Some)
        .map_err(|_| "Max parallel agents must be a positive integer.".to_string())
}

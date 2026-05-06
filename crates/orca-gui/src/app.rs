use std::collections::HashMap;
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
    page: AppPage,
    workflow_content: String,
    workflow_path: String,
    new_workflow_path: String,
    selected_workflow_node: Option<String>,
    selected_workflow_connection: Option<WorkflowCanvasConnection>,
    workflow_goal_connections: Vec<String>,
    workflow_node_positions: HashMap<String, egui::Pos2>,
    workflow_connection_drag: Option<WorkflowConnectionDrag>,
    workflow_canvas_pan: egui::Vec2,
    workflow_canvas_pan_start: Option<egui::Vec2>,
    artifact_roots: Vec<String>,
    selected_artifact_root: String,
    artifact_files: Vec<String>,
    selected_artifact_file: String,
    artifact_content: String,
    page_status: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppPage {
    Run,
    Workflows,
    Artifacts,
}

#[derive(Debug, Clone)]
struct WorkflowNodeView {
    id: String,
    kind: String,
    evaluation: String,
    contract: String,
    instruction: String,
    depends_on: Vec<String>,
    inputs: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CanvasNodeKind {
    Goal,
    Agent,
    Branch,
}

const GOAL_NODE_ID: &str = "__goal__";

#[derive(Debug, Clone)]
struct WorkflowCanvasNode {
    id: String,
    kind: CanvasNodeKind,
    label: String,
    detail: String,
    agent_id: Option<String>,
    inputs: Vec<String>,
    rect: egui::Rect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkflowCanvasConnection {
    from_id: String,
    to_id: String,
    condition: Option<String>,
    input_index: Option<usize>,
}

#[derive(Debug, Clone)]
struct WorkflowConnectionDrag {
    from_id: String,
    condition: Option<String>,
}

#[derive(Debug, Clone)]
struct WorkflowConnectionView {
    from_id: String,
    to_ids: Vec<String>,
    condition: Option<String>,
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
            page: AppPage::Run,
            workflow_content: String::new(),
            workflow_path: String::new(),
            new_workflow_path: String::new(),
            selected_workflow_node: None,
            selected_workflow_connection: None,
            workflow_goal_connections: Vec::new(),
            workflow_node_positions: HashMap::new(),
            workflow_connection_drag: None,
            workflow_canvas_pan: egui::Vec2::ZERO,
            workflow_canvas_pan_start: None,
            artifact_roots: Vec::new(),
            selected_artifact_root: String::new(),
            artifact_files: Vec::new(),
            selected_artifact_file: String::new(),
            artifact_content: String::new(),
            page_status: None,
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
                ui.separator();
                ui.selectable_value(&mut self.page, AppPage::Run, "Run");
                ui.selectable_value(&mut self.page, AppPage::Workflows, "Workflows");
                ui.selectable_value(&mut self.page, AppPage::Artifacts, "Artifacts");
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

        match self.page {
            AppPage::Run => self.draw_run_page(ctx),
            AppPage::Workflows => self.draw_workflows_page(ctx),
            AppPage::Artifacts => self.draw_artifacts_page(ctx),
        }
    }
}

impl OrcaApp {
    fn draw_run_page(&mut self, ctx: &egui::Context) {
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

    fn draw_workflows_page(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("workflow-files")
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("Workflows");
                let configs = self.settings.workflow_configs();
                egui::ComboBox::from_id_salt("workflow-page-config")
                    .selected_text(if self.workflow_path.is_empty() {
                        "Select workflow".to_string()
                    } else {
                        self.workflow_path.clone()
                    })
                    .show_ui(ui, |ui| {
                        for config in configs {
                            let value = config.display().to_string();
                            ui.selectable_value(&mut self.workflow_path, value.clone(), value);
                        }
                    });
                ui.horizontal(|ui| {
                    if ui.button("Load").clicked() {
                        self.load_workflow_from_page();
                    }
                    if ui.button("Save").clicked() {
                        self.save_workflow_from_page();
                    }
                });
                ui.label("New or duplicate path");
                ui.text_edit_singleline(&mut self.new_workflow_path);
                ui.horizontal(|ui| {
                    if ui.button("New").clicked() {
                        self.create_workflow_from_page(false);
                    }
                    if ui.button("Duplicate").clicked() {
                        self.create_workflow_from_page(true);
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("Add agent").clicked() {
                        self.add_workflow_node();
                    }
                    if ui.button("Add branch").clicked() {
                        self.add_workflow_branch_node();
                    }
                });
                ui.separator();
                ui.heading("Keyboard");
                ui.small("Drag blocks to move them");
                ui.small("Drag circles to connect inputs");
                ui.small("Click a line to select it");
                ui.small("Delete removes the selected line or agent");
                if let Some(status) = &self.page_status {
                    ui.small(status);
                }
            });

        egui::SidePanel::right("workflow-source")
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("Node inspector");
                self.draw_selected_workflow_node(ui);
                ui.separator();
                ui.heading("Workflow source");
                ui.add(egui::TextEdit::multiline(&mut self.workflow_content).code_editor());
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.draw_workflow_canvas(ui);
        });
    }

    fn draw_artifacts_page(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("artifact-runs")
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("Artifacts");
                if ui.button("Load runs").clicked() {
                    self.load_artifact_roots();
                }
                egui::ComboBox::from_id_salt("artifact-root")
                    .selected_text(if self.selected_artifact_root.is_empty() {
                        "Select run".to_string()
                    } else {
                        self.selected_artifact_root.clone()
                    })
                    .show_ui(ui, |ui| {
                        for root in &self.artifact_roots {
                            ui.selectable_value(
                                &mut self.selected_artifact_root,
                                root.clone(),
                                root,
                            );
                        }
                    });
                if ui.button("Load files").clicked() {
                    self.load_artifact_files();
                }
                egui::ComboBox::from_id_salt("artifact-file")
                    .selected_text(if self.selected_artifact_file.is_empty() {
                        "Select file".to_string()
                    } else {
                        self.selected_artifact_file.clone()
                    })
                    .show_ui(ui, |ui| {
                        for file in &self.artifact_files {
                            ui.selectable_value(
                                &mut self.selected_artifact_file,
                                file.clone(),
                                file,
                            );
                        }
                    });
                if ui.button("Read").clicked() {
                    self.read_selected_artifact();
                }
                if let Some(status) = &self.page_status {
                    ui.small(status);
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Artifact explorer");
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add(
                    egui::Label::new(egui::RichText::new(&self.artifact_content).monospace())
                        .wrap(),
                );
            });
        });
    }

    fn draw_workflow_canvas(&mut self, ui: &mut egui::Ui) {
        if ui.input(|input| input.key_pressed(egui::Key::Delete))
            && !ui.ctx().wants_keyboard_input()
        {
            self.delete_workflow_canvas_selection();
        }

        let nodes = parse_workflow_nodes(&self.workflow_content);
        let connections = parse_workflow_connections(&self.workflow_content);
        let (canvas_rect, canvas_response) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
        let painter = ui.painter_at(canvas_rect);
        painter.rect_filled(canvas_rect, 4.0, egui::Color32::from_rgb(248, 251, 255));
        let grid_x = canvas_rect.left() + self.workflow_canvas_pan.x.rem_euclid(32.0);
        let grid_y = canvas_rect.top() + self.workflow_canvas_pan.y.rem_euclid(32.0);
        let mut x = grid_x;
        while x <= canvas_rect.right() {
            painter.line_segment(
                [
                    egui::pos2(x, canvas_rect.top()),
                    egui::pos2(x, canvas_rect.bottom()),
                ],
                egui::Stroke::new(1.0, egui::Color32::from_rgb(219, 234, 254)),
            );
            x += 32.0;
        }
        let mut y = grid_y;
        while y <= canvas_rect.bottom() {
            painter.line_segment(
                [
                    egui::pos2(canvas_rect.left(), y),
                    egui::pos2(canvas_rect.right(), y),
                ],
                egui::Stroke::new(1.0, egui::Color32::from_rgb(219, 234, 254)),
            );
            y += 32.0;
        }

        let mut canvas_nodes = Vec::new();
        canvas_nodes.push(self.workflow_canvas_node(
            canvas_rect,
            GOAL_NODE_ID,
            CanvasNodeKind::Goal,
            "Main goal",
            "workflow goal",
            None,
            Vec::new(),
            egui::pos2(42.0, 72.0),
            egui::vec2(170.0, 88.0),
        ));
        for (index, node) in nodes.iter().enumerate() {
            let kind = if node.kind == "branch" {
                CanvasNodeKind::Branch
            } else {
                CanvasNodeKind::Agent
            };
            canvas_nodes.push(self.workflow_canvas_node(
                canvas_rect,
                &node.id,
                kind,
                &node.id,
                if kind == CanvasNodeKind::Branch {
                    "true / false"
                } else {
                    &node.contract
                },
                Some(node.id.clone()),
                node.inputs.clone(),
                egui::pos2(260.0, 72.0 + index as f32 * 132.0),
                egui::vec2(190.0, 96.0),
            ));
        }

        let goal_node = canvas_nodes
            .iter()
            .find(|node| node.kind == CanvasNodeKind::Goal);
        let mut selectable_edges = Vec::new();
        for node in &nodes {
            let Some(to_node) = canvas_nodes
                .iter()
                .find(|canvas_node| canvas_node.id == node.id)
            else {
                continue;
            };
            for (input_index, source) in node.inputs.iter().enumerate() {
                let Some(from_node) = canvas_nodes.iter().find(|canvas_node| {
                    if source == "goal" {
                        canvas_node.id == GOAL_NODE_ID
                    } else {
                        canvas_node.id == *source
                    }
                }) else {
                    continue;
                };
                let edge = WorkflowCanvasConnection {
                    from_id: if source == "goal" {
                        GOAL_NODE_ID.to_string()
                    } else {
                        source.clone()
                    },
                    to_id: node.id.clone(),
                    condition: None,
                    input_index: Some(input_index),
                };
                let selected = self.selected_workflow_connection.as_ref() == Some(&edge);
                draw_canvas_edge(
                    &painter,
                    output_handle_for_connection(from_node, edge.condition.as_deref()),
                    input_handle_at(to_node, input_index),
                    if selected {
                        egui::Color32::from_rgb(249, 115, 22)
                    } else if edge.from_id == GOAL_NODE_ID {
                        egui::Color32::from_rgb(234, 179, 8)
                    } else {
                        egui::Color32::from_rgb(14, 165, 233)
                    },
                    if selected { 4.0 } else { 2.0 },
                );
                selectable_edges.push((
                    edge.clone(),
                    output_handle_for_connection(from_node, edge.condition.as_deref()),
                    input_handle_at(to_node, input_index),
                ));
            }
            for dependency in &node.depends_on {
                if node.inputs.iter().any(|source| source == dependency) {
                    continue;
                }
                let Some(from_node) = canvas_nodes
                    .iter()
                    .find(|canvas_node| canvas_node.id == *dependency)
                else {
                    continue;
                };
                let edge = WorkflowCanvasConnection {
                    from_id: dependency.clone(),
                    to_id: node.id.clone(),
                    condition: None,
                    input_index: None,
                };
                let selected = self.selected_workflow_connection.as_ref() == Some(&edge);
                draw_canvas_edge(
                    &painter,
                    output_handle_for_connection(from_node, edge.condition.as_deref()),
                    input_handle(to_node),
                    if selected {
                        egui::Color32::from_rgb(249, 115, 22)
                    } else {
                        egui::Color32::from_rgb(14, 165, 233)
                    },
                    if selected { 4.0 } else { 2.0 },
                );
                selectable_edges.push((
                    edge.clone(),
                    output_handle_for_connection(from_node, edge.condition.as_deref()),
                    input_handle(to_node),
                ));
            }
        }
        for connection in &connections {
            let Some(from_node) = canvas_nodes
                .iter()
                .find(|canvas_node| canvas_node.id == connection.from_id)
            else {
                continue;
            };
            for to_id in &connection.to_ids {
                let Some(to_node) = canvas_nodes
                    .iter()
                    .find(|canvas_node| canvas_node.id == *to_id)
                else {
                    continue;
                };
                let edge = WorkflowCanvasConnection {
                    from_id: connection.from_id.clone(),
                    to_id: to_id.clone(),
                    condition: connection.condition.clone(),
                    input_index: None,
                };
                let selected = self.selected_workflow_connection.as_ref() == Some(&edge);
                draw_canvas_edge(
                    &painter,
                    output_handle_for_connection(from_node, edge.condition.as_deref()),
                    input_handle(to_node),
                    if selected {
                        egui::Color32::from_rgb(249, 115, 22)
                    } else {
                        egui::Color32::from_rgb(14, 165, 233)
                    },
                    if selected { 4.0 } else { 2.0 },
                );
                if let Some(condition) = &connection.condition {
                    draw_canvas_edge_label(
                        &painter,
                        output_handle_for_connection(from_node, Some(condition)),
                        input_handle(to_node),
                        condition,
                    );
                }
                selectable_edges.push((
                    edge.clone(),
                    output_handle_for_connection(from_node, edge.condition.as_deref()),
                    input_handle(to_node),
                ));
            }
        }
        for node in &nodes {
            let Some(to_node) = canvas_nodes
                .iter()
                .find(|canvas_node| canvas_node.id == node.id)
            else {
                continue;
            };
            if node.depends_on.is_empty()
                && !node.inputs.iter().any(|source| source == "goal")
                && self
                    .workflow_goal_connections
                    .iter()
                    .any(|connection| connection == &node.id)
                && let Some(from_node) = goal_node
            {
                let edge = WorkflowCanvasConnection {
                    from_id: GOAL_NODE_ID.to_string(),
                    to_id: node.id.clone(),
                    condition: None,
                    input_index: None,
                };
                let selected = self.selected_workflow_connection.as_ref() == Some(&edge);
                draw_canvas_edge(
                    &painter,
                    output_handle_for_connection(from_node, edge.condition.as_deref()),
                    input_handle(to_node),
                    if selected {
                        egui::Color32::from_rgb(249, 115, 22)
                    } else {
                        egui::Color32::from_rgb(234, 179, 8)
                    },
                    if selected { 4.0 } else { 2.0 },
                );
                selectable_edges.push((
                    edge.clone(),
                    output_handle_for_connection(from_node, edge.condition.as_deref()),
                    input_handle(to_node),
                ));
            }
        }

        if canvas_response.clicked()
            && let Some(pointer) = canvas_response.interact_pointer_pos()
        {
            self.selected_workflow_connection = selectable_edges
                .iter()
                .find(|(_, from, to)| distance_to_segment(pointer, *from, *to) <= 8.0)
                .map(|(edge, _, _)| edge.clone());
            if self.selected_workflow_connection.is_some() {
                self.selected_workflow_node = None;
            }
        }

        if canvas_response.drag_started()
            && let Some(pointer) = canvas_response.interact_pointer_pos()
            && !canvas_pointer_hits_scene(pointer, &canvas_nodes, &selectable_edges)
        {
            self.workflow_canvas_pan_start = Some(self.workflow_canvas_pan);
            self.selected_workflow_connection = None;
        }
        if canvas_response.dragged()
            && let Some(start_pan) = self.workflow_canvas_pan_start
        {
            self.workflow_canvas_pan = start_pan + canvas_response.drag_delta();
        }
        if canvas_response.drag_stopped() {
            self.workflow_canvas_pan_start = None;
        }

        if let Some(drag) = &self.workflow_connection_drag
            && let Some(from_node) = canvas_nodes.iter().find(|node| node.id == drag.from_id)
            && let Some(pointer) = ui.input(|input| input.pointer.hover_pos())
        {
            draw_canvas_edge(
                &painter,
                output_handle_for_connection(from_node, drag.condition.as_deref()),
                pointer,
                egui::Color32::from_rgb(249, 115, 22),
                2.0,
            );
        }

        let hovered_connection_target = self.workflow_connection_drag.as_ref().and_then(|drag| {
            ui.input(|input| input.pointer.hover_pos())
                .and_then(|pointer| {
                    find_connection_target(&canvas_nodes, &drag.from_id, pointer)
                        .map(|(target_id, _)| target_id)
                })
        });

        let mut pending_connection: Option<(String, String, Option<String>, Option<usize>)> = None;
        for canvas_node in &canvas_nodes {
            let selected = canvas_node.kind != CanvasNodeKind::Goal
                && self.selected_workflow_node.as_deref() == canvas_node.agent_id.as_deref();
            let connection_target = hovered_connection_target.as_deref() == Some(&canvas_node.id);
            painter.rect_filled(
                canvas_node.rect,
                8.0,
                canvas_node_fill(canvas_node.kind, selected || connection_target),
            );
            painter.rect_stroke(
                canvas_node.rect,
                8.0,
                egui::Stroke::new(
                    if selected || connection_target {
                        3.0
                    } else {
                        1.0
                    },
                    if connection_target {
                        egui::Color32::from_rgb(249, 115, 22)
                    } else {
                        canvas_node_stroke(canvas_node.kind, selected)
                    },
                ),
            );
            painter.text(
                canvas_node.rect.left_top() + egui::vec2(12.0, 14.0),
                egui::Align2::LEFT_TOP,
                &canvas_node.label,
                egui::FontId::proportional(14.0),
                egui::Color32::from_rgb(15, 23, 42),
            );
            painter.text(
                canvas_node.rect.left_top() + egui::vec2(12.0, 40.0),
                egui::Align2::LEFT_TOP,
                &canvas_node.detail,
                egui::FontId::proportional(12.0),
                if canvas_node.kind == CanvasNodeKind::Goal {
                    egui::Color32::from_rgb(133, 77, 14)
                } else if canvas_node.kind == CanvasNodeKind::Branch {
                    egui::Color32::from_rgb(161, 98, 7)
                } else {
                    egui::Color32::from_rgb(3, 105, 161)
                },
            );
            if canvas_node.kind != CanvasNodeKind::Goal {
                draw_input_handles(
                    &painter,
                    canvas_node,
                    canvas_node_stroke(canvas_node.kind, selected),
                );
            }
            draw_output_handles(&painter, canvas_node);

            let response = ui.interact(
                canvas_node.rect,
                ui.make_persistent_id(("workflow-canvas-node", &canvas_node.id)),
                egui::Sense::click_and_drag(),
            );
            if response.clicked() && canvas_node.kind != CanvasNodeKind::Goal {
                self.selected_workflow_node = canvas_node.agent_id.clone();
                self.selected_workflow_connection = None;
            }
            if response.dragged() {
                let entry = self
                    .workflow_node_positions
                    .entry(canvas_node.id.clone())
                    .or_insert_with(|| {
                        canvas_node.rect.min - canvas_rect.min.to_vec2() - self.workflow_canvas_pan
                    });
                *entry += response.drag_delta();
            }

            for (handle_index, handle_condition, handle_position) in output_handles(canvas_node) {
                let output_rect =
                    egui::Rect::from_center_size(handle_position, egui::vec2(24.0, 24.0));
                let output_response = ui.interact(
                    output_rect,
                    ui.make_persistent_id((
                        "workflow-canvas-output",
                        &canvas_node.id,
                        handle_index,
                    )),
                    egui::Sense::drag(),
                );
                if output_response.drag_started() {
                    self.workflow_connection_drag = Some(WorkflowConnectionDrag {
                        from_id: canvas_node.id.clone(),
                        condition: handle_condition.clone(),
                    });
                    self.selected_workflow_connection = None;
                }
                if output_response.drag_stopped() {
                    if let Some(drag) = self.workflow_connection_drag.take()
                        && let Some(pointer) = output_response.interact_pointer_pos()
                        && let Some((target_id, input_index)) =
                            find_connection_target(&canvas_nodes, &drag.from_id, pointer)
                    {
                        pending_connection =
                            Some((drag.from_id, target_id, drag.condition, input_index));
                    }
                }
            }
        }

        if let Some((from_id, to_id, condition, input_index)) = pending_connection {
            self.connect_workflow_canvas_nodes(&from_id, &to_id, condition.as_deref(), input_index);
        }
    }

    fn workflow_canvas_node(
        &mut self,
        canvas_rect: egui::Rect,
        id: &str,
        kind: CanvasNodeKind,
        label: &str,
        detail: &str,
        agent_id: Option<String>,
        inputs: Vec<String>,
        default_position: egui::Pos2,
        size: egui::Vec2,
    ) -> WorkflowCanvasNode {
        let position = *self
            .workflow_node_positions
            .entry(id.to_string())
            .or_insert(default_position);
        WorkflowCanvasNode {
            id: id.to_string(),
            kind,
            label: label.to_string(),
            detail: detail.to_string(),
            agent_id,
            inputs,
            rect: egui::Rect::from_min_size(
                canvas_rect.min + self.workflow_canvas_pan + position.to_vec2(),
                size,
            ),
        }
    }

    fn connect_workflow_canvas_nodes(
        &mut self,
        from_id: &str,
        to_id: &str,
        condition: Option<&str>,
        input_index: Option<usize>,
    ) {
        let nodes = parse_workflow_nodes(&self.workflow_content);
        let Some(to_agent) = nodes
            .iter()
            .find(|node| node.id == to_id)
            .map(|node| node.id.clone())
        else {
            return;
        };
        if from_id == GOAL_NODE_ID {
            self.workflow_content =
                set_workflow_node_dependencies(&self.workflow_content, &to_agent, &[]);
            self.workflow_content =
                add_workflow_node_input(&self.workflow_content, &to_agent, "goal", input_index);
            if !self
                .workflow_goal_connections
                .iter()
                .any(|connection| connection == &to_agent)
            {
                self.workflow_goal_connections.push(to_agent.clone());
            }
            self.selected_workflow_node = Some(to_agent.clone());
            self.selected_workflow_connection = Some(WorkflowCanvasConnection {
                from_id: GOAL_NODE_ID.to_string(),
                to_id: to_agent,
                condition: None,
                input_index,
            });
            return;
        }
        let from_agent = from_id;
        if from_agent != to_agent {
            self.workflow_goal_connections
                .retain(|connection| connection != &to_agent);
            let from_kind = nodes
                .iter()
                .find(|node| node.id == from_agent)
                .map(|node| node.kind.as_str())
                .unwrap_or("agent");
            let condition = if from_kind == "branch" {
                let condition = condition
                    .map(str::to_string)
                    .unwrap_or_else(|| next_branch_condition(&self.workflow_content, from_agent));
                self.workflow_content = add_workflow_connection(
                    &self.workflow_content,
                    from_agent,
                    &to_agent,
                    Some(&condition),
                );
                Some(condition)
            } else {
                self.workflow_content =
                    add_workflow_node_dependency(&self.workflow_content, &to_agent, from_agent);
                self.workflow_content = add_workflow_node_input(
                    &self.workflow_content,
                    &to_agent,
                    from_agent,
                    input_index,
                );
                None
            };
            self.selected_workflow_node = Some(to_agent.clone());
            self.selected_workflow_connection = Some(WorkflowCanvasConnection {
                from_id: from_agent.to_string(),
                to_id: to_agent,
                condition,
                input_index,
            });
        }
    }

    fn delete_workflow_canvas_selection(&mut self) {
        if let Some(connection) = self.selected_workflow_connection.take() {
            if connection.from_id == GOAL_NODE_ID {
                self.workflow_goal_connections
                    .retain(|goal_connection| goal_connection != &connection.to_id);
                self.workflow_content =
                    remove_workflow_node_input(&self.workflow_content, &connection.to_id, "goal");
            } else {
                self.workflow_content = if connection.condition.is_some() {
                    remove_workflow_connection(
                        &self.workflow_content,
                        &connection.from_id,
                        &connection.to_id,
                        connection.condition.as_deref(),
                    )
                } else {
                    let content = remove_workflow_node_dependency(
                        &self.workflow_content,
                        &connection.to_id,
                        &connection.from_id,
                    );
                    remove_workflow_node_input(&content, &connection.to_id, &connection.from_id)
                };
            }
            return;
        }
        if let Some(node_id) = self.selected_workflow_node.take() {
            self.workflow_content = remove_workflow_node(&self.workflow_content, &node_id);
            self.workflow_node_positions.remove(&node_id);
            self.workflow_goal_connections
                .retain(|goal_connection| goal_connection != &node_id);
        }
    }

    fn draw_selected_workflow_node(&mut self, ui: &mut egui::Ui) {
        let nodes = parse_workflow_nodes(&self.workflow_content);
        let Some(selected_id) = self
            .selected_workflow_node
            .clone()
            .or_else(|| nodes.first().map(|node| node.id.clone()))
        else {
            ui.label("No node selected.");
            return;
        };
        if let Some(node) = nodes.iter().find(|node| node.id == selected_id) {
            ui.label(format!("Selected: {}", node.id));
            ui.small(format!("Contract: {}", node.contract));
            ui.small(format!("Instruction: {}", node.instruction));
            if node.kind == "branch" {
                ui.small(format!("Evaluation: {}", node.evaluation));
            }
            if !node.inputs.is_empty() {
                ui.small(format!("Inputs: {}", node.inputs.join(" -> ")));
            }
            if ui.button("Remove selected node").clicked() {
                self.workflow_content = remove_workflow_node(&self.workflow_content, &node.id);
                self.selected_workflow_node = None;
                self.selected_workflow_connection = None;
                self.workflow_goal_connections
                    .retain(|goal_connection| goal_connection != &node.id);
            }
        }
    }

    fn load_workflow_from_page(&mut self) {
        let path = optional_path(&self.workflow_path)
            .unwrap_or_else(|| self.settings.resolve_workflow_config(None));
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                self.workflow_path = path.display().to_string();
                self.workflow_content = content;
                self.workflow_goal_connections.clear();
                self.selected_workflow_node = parse_workflow_nodes(&self.workflow_content)
                    .first()
                    .map(|node| node.id.clone());
                self.page_status = Some("Workflow loaded".to_string());
            }
            Err(error) => {
                self.page_status = Some(format!("failed to read `{}`: {error}", path.display()))
            }
        }
    }

    fn save_workflow_from_page(&mut self) {
        let Some(path) = optional_path(&self.workflow_path) else {
            self.page_status = Some("Select a workflow path before saving".to_string());
            return;
        };
        if let Some(parent) = path.parent()
            && let Err(error) = std::fs::create_dir_all(parent)
        {
            self.page_status = Some(format!("failed to create `{}`: {error}", parent.display()));
            return;
        }
        match std::fs::write(&path, &self.workflow_content) {
            Ok(()) => self.page_status = Some("Workflow saved".to_string()),
            Err(error) => {
                self.page_status = Some(format!("failed to write `{}`: {error}", path.display()))
            }
        }
    }

    fn create_workflow_from_page(&mut self, duplicate: bool) {
        let Some(path) = optional_path(&self.new_workflow_path) else {
            self.page_status = Some("Enter a new workflow path".to_string());
            return;
        };
        if !duplicate || self.workflow_content.trim().is_empty() {
            self.workflow_content = default_workflow_content();
            self.workflow_goal_connections.clear();
        }
        self.workflow_path = path.display().to_string();
        self.save_workflow_from_page();
    }

    fn add_workflow_node(&mut self) {
        let nodes = parse_workflow_nodes(&self.workflow_content);
        let id = unique_workflow_node_id(&nodes);
        if self.workflow_content.trim().is_empty() {
            self.workflow_content =
                "[orchestration]\nmax_parallel_agents = 8\napproval_mode = \"auto\"\n\n"
                    .to_string();
        }
        self.workflow_content.push_str(&format!(
            "[[orchestration.nodes]]\nid = \"{id}\"\noutput_contract = \"implementation\"\ninstruction = \"work.md\"\n\n"
        ));
        self.selected_workflow_node = Some(id);
        self.selected_workflow_connection = None;
    }

    fn add_workflow_branch_node(&mut self) {
        let nodes = parse_workflow_nodes(&self.workflow_content);
        let id = unique_prefixed_workflow_node_id(&nodes, "branch");
        if self.workflow_content.trim().is_empty() {
            self.workflow_content =
                "[orchestration]\nmax_parallel_agents = 8\napproval_mode = \"auto\"\n\n"
                    .to_string();
        }
        self.workflow_content.push_str(&format!(
            "[[orchestration.nodes]]\nid = \"{id}\"\nkind = \"branch\"\nevaluation = \"input contains signed_off = true\"\noutput_contract = \"branch\"\n\n"
        ));
        self.selected_workflow_node = Some(id);
        self.selected_workflow_connection = None;
    }

    fn load_artifact_roots(&mut self) {
        let artifact_dir = self
            .settings
            .default_artifact_dir()
            .unwrap_or_else(|| PathBuf::from("orca-runs"));
        match std::fs::read_dir(&artifact_dir) {
            Ok(entries) => {
                self.artifact_roots = entries
                    .filter_map(Result::ok)
                    .map(|entry| entry.path())
                    .filter(|path| path.is_dir())
                    .map(|path| path.display().to_string())
                    .collect();
                self.artifact_roots.sort();
                if self.selected_artifact_root.is_empty() {
                    self.selected_artifact_root =
                        self.artifact_roots.first().cloned().unwrap_or_default();
                }
                self.page_status = Some(format!("Loaded {} run(s)", self.artifact_roots.len()));
            }
            Err(error) => {
                self.page_status = Some(format!(
                    "failed to read `{}`: {error}",
                    artifact_dir.display()
                ))
            }
        }
    }

    fn load_artifact_files(&mut self) {
        self.artifact_files.clear();
        collect_files(
            PathBuf::from(&self.selected_artifact_root),
            &mut self.artifact_files,
        );
        self.artifact_files.sort();
        self.selected_artifact_file = self
            .artifact_files
            .iter()
            .find(|file| file.ends_with("manifest.json"))
            .cloned()
            .or_else(|| self.artifact_files.first().cloned())
            .unwrap_or_default();
        self.page_status = Some(format!("Loaded {} file(s)", self.artifact_files.len()));
    }

    fn read_selected_artifact(&mut self) {
        let path = if self.selected_artifact_file.is_empty() {
            &self.selected_artifact_root
        } else {
            &self.selected_artifact_file
        };
        match std::fs::read_to_string(path) {
            Ok(content) => self.artifact_content = content,
            Err(error) => self.page_status = Some(format!("failed to read `{path}`: {error}")),
        }
    }

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

fn parse_workflow_nodes(content: &str) -> Vec<WorkflowNodeView> {
    content
        .split("[[orchestration.nodes]]")
        .skip(1)
        .map(|block| WorkflowNodeView {
            id: string_field(block, "id").unwrap_or_else(|| "agent".to_string()),
            kind: string_field(block, "kind").unwrap_or_else(|| "agent".to_string()),
            evaluation: string_field(block, "evaluation").unwrap_or_default(),
            contract: string_field(block, "output_contract")
                .unwrap_or_else(|| "implementation".to_string()),
            instruction: string_field(block, "instruction").unwrap_or_default(),
            depends_on: array_field(block, "depends_on"),
            inputs: input_sources_field(block),
        })
        .collect()
}

fn input_sources_field(block: &str) -> Vec<String> {
    let Some(start) = block.find("inputs") else {
        return Vec::new();
    };
    let value = &block[start..];
    let Some(open) = value.find('[') else {
        return Vec::new();
    };
    let Some(close) = value[open..].find(']') else {
        return Vec::new();
    };
    value[open + 1..open + close]
        .split("source")
        .skip(1)
        .filter_map(|source| {
            source
                .split_once('"')
                .and_then(|(_, rest)| rest.split_once('"'))
                .map(|(source, _)| source.to_string())
        })
        .collect()
}

fn parse_workflow_connections(content: &str) -> Vec<WorkflowConnectionView> {
    content
        .split("[[orchestration.connections]]")
        .skip(1)
        .filter_map(|block| {
            let from_id = string_field(block, "from")?;
            let to_ids = array_field(block, "to");
            (!to_ids.is_empty()).then(|| WorkflowConnectionView {
                from_id,
                to_ids,
                condition: string_field(block, "condition"),
            })
        })
        .collect()
}

fn string_field(block: &str, key: &str) -> Option<String> {
    block.lines().find_map(|line| {
        let line = line.trim();
        let value = line
            .strip_prefix(key)?
            .trim_start()
            .strip_prefix('=')?
            .trim();
        value
            .strip_prefix('"')
            .and_then(|value| value.split('"').next())
            .map(str::to_string)
    })
}

fn array_field(block: &str, key: &str) -> Vec<String> {
    block
        .lines()
        .find_map(|line| {
            let line = line.trim();
            let value = line
                .strip_prefix(key)?
                .trim_start()
                .strip_prefix('=')?
                .trim();
            Some(
                value
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .split(',')
                    .filter_map(|item| {
                        let item = item.trim().trim_matches('"');
                        (!item.is_empty()).then(|| item.to_string())
                    })
                    .collect(),
            )
        })
        .unwrap_or_default()
}

fn default_workflow_content() -> String {
    "[orchestration]\nmax_parallel_agents = 8\napproval_mode = \"auto\"\n\n[[orchestration.nodes]]\nid = \"work-agent\"\noutput_contract = \"implementation\"\ninstruction = \"work.md\"\n\n"
        .to_string()
}

fn unique_workflow_node_id(nodes: &[WorkflowNodeView]) -> String {
    unique_prefixed_workflow_node_id(nodes, "agent")
}

fn unique_prefixed_workflow_node_id(nodes: &[WorkflowNodeView], prefix: &str) -> String {
    let mut index = nodes.len() + 1;
    loop {
        let id = format!("{prefix}-{index}");
        if !nodes.iter().any(|node| node.id == id) {
            return id;
        }
        index += 1;
    }
}

fn next_branch_condition(content: &str, from_id: &str) -> String {
    let used = parse_workflow_connections(content)
        .into_iter()
        .filter(|connection| connection.from_id == from_id)
        .filter_map(|connection| connection.condition)
        .collect::<Vec<_>>();
    if used.iter().any(|condition| condition == "true")
        && !used.iter().any(|condition| condition == "false")
    {
        "false".to_string()
    } else {
        "true".to_string()
    }
}

fn add_workflow_connection(
    content: &str,
    from_id: &str,
    to_id: &str,
    condition: Option<&str>,
) -> String {
    let mut output = String::new();
    let mut first = true;
    let mut inserted = false;
    for block in content.split("[[orchestration.connections]]") {
        if first {
            output.push_str(block);
            first = false;
            continue;
        }
        let block_from = string_field(block, "from");
        let block_condition = string_field(block, "condition");
        output.push_str("[[orchestration.connections]]");
        if block_from.as_deref() == Some(from_id) && block_condition.as_deref() == condition {
            let mut targets = array_field(block, "to");
            if !targets.iter().any(|target| target == to_id) {
                targets.push(to_id.to_string());
            }
            output.push_str(&replace_array_field(block, "to", &targets));
            inserted = true;
        } else {
            output.push_str(block);
        }
    }
    if !inserted {
        if !output.ends_with('\n') {
            output.push('\n');
        }
        output.push_str("\n[[orchestration.connections]]\n");
        output.push_str(&format!("from = \"{}\"\n", escape_toml(from_id)));
        if let Some(condition) = condition {
            output.push_str(&format!("condition = \"{}\"\n", escape_toml(condition)));
        }
        output.push_str(&format!("to = [\"{}\"]\n", escape_toml(to_id)));
    }
    output
}

fn remove_workflow_connection(
    content: &str,
    from_id: &str,
    to_id: &str,
    condition: Option<&str>,
) -> String {
    let mut output = String::new();
    let mut first = true;
    for block in content.split("[[orchestration.connections]]") {
        if first {
            output.push_str(block);
            first = false;
            continue;
        }
        let block_from = string_field(block, "from");
        let block_condition = string_field(block, "condition");
        if block_from.as_deref() == Some(from_id) && block_condition.as_deref() == condition {
            let targets = array_field(block, "to")
                .into_iter()
                .filter(|target| target != to_id)
                .collect::<Vec<_>>();
            if !targets.is_empty() {
                output.push_str("[[orchestration.connections]]");
                output.push_str(&replace_array_field(block, "to", &targets));
            }
        } else {
            output.push_str("[[orchestration.connections]]");
            output.push_str(block);
        }
    }
    output
}

fn replace_array_field(block: &str, key: &str, values: &[String]) -> String {
    let replacement = format!(
        "{key} = [{}]",
        values
            .iter()
            .map(|value| format!("\"{}\"", escape_toml(value)))
            .collect::<Vec<_>>()
            .join(", ")
    );
    block
        .lines()
        .map(|line| {
            if line.trim_start().starts_with(key) {
                replacement.clone()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + if block.ends_with('\n') { "\n" } else { "" }
}

fn escape_toml(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn remove_workflow_node(content: &str, id: &str) -> String {
    let mut output = String::new();
    let mut first = true;
    for block in content.split("[[orchestration.nodes]]") {
        if first {
            output.push_str(block);
            first = false;
            continue;
        }
        if string_field(block, "id").as_deref() != Some(id) {
            output.push_str("[[orchestration.nodes]]");
            output.push_str(block);
        }
    }
    output
}

fn remove_workflow_node_dependency(content: &str, node_id: &str, dependency: &str) -> String {
    let dependencies = parse_workflow_nodes(content)
        .into_iter()
        .find(|node| node.id == node_id)
        .map(|node| {
            node.depends_on
                .into_iter()
                .filter(|existing| existing != dependency)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    set_workflow_node_dependencies(content, node_id, &dependencies)
}

fn add_workflow_node_dependency(content: &str, node_id: &str, dependency: &str) -> String {
    let mut dependencies = parse_workflow_nodes(content)
        .into_iter()
        .find(|node| node.id == node_id)
        .map(|node| node.depends_on)
        .unwrap_or_default();
    if !dependencies.iter().any(|existing| existing == dependency) {
        dependencies.push(dependency.to_string());
    }
    set_workflow_node_dependencies(content, node_id, &dependencies)
}

fn add_workflow_node_input(
    content: &str,
    node_id: &str,
    source: &str,
    input_index: Option<usize>,
) -> String {
    let mut inputs = parse_workflow_nodes(content)
        .into_iter()
        .find(|node| node.id == node_id)
        .map(|node| node.inputs)
        .unwrap_or_default()
        .into_iter()
        .filter(|existing| existing != source)
        .collect::<Vec<_>>();
    let index = input_index.unwrap_or(inputs.len()).min(inputs.len());
    inputs.insert(index, source.to_string());
    set_workflow_node_inputs(content, node_id, &inputs)
}

fn remove_workflow_node_input(content: &str, node_id: &str, source: &str) -> String {
    let inputs = parse_workflow_nodes(content)
        .into_iter()
        .find(|node| node.id == node_id)
        .map(|node| {
            node.inputs
                .into_iter()
                .filter(|existing| existing != source)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    set_workflow_node_inputs(content, node_id, &inputs)
}

fn set_workflow_node_inputs(content: &str, node_id: &str, inputs: &[String]) -> String {
    let mut output = String::new();
    let mut first = true;
    for block in content.split("[[orchestration.nodes]]") {
        if first {
            output.push_str(block);
            first = false;
            continue;
        }
        output.push_str("[[orchestration.nodes]]");
        if string_field(block, "id").as_deref() == Some(node_id) {
            let mut lines = block
                .lines()
                .filter(|line| !line.trim_start().starts_with("inputs"))
                .map(str::to_string)
                .collect::<Vec<_>>();
            if !inputs.is_empty() {
                lines.push(format!(
                    "inputs = [{}]",
                    inputs
                        .iter()
                        .map(|source| format!("{{ source = \"{}\" }}", escape_toml(source)))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            output.push_str(&lines.join("\n"));
            if block.ends_with('\n') {
                output.push('\n');
            }
        } else {
            output.push_str(block);
        }
    }
    output
}

fn set_workflow_node_dependencies(content: &str, node_id: &str, dependencies: &[String]) -> String {
    let mut output = String::new();
    let mut first = true;
    for block in content.split("[[orchestration.nodes]]") {
        if first {
            output.push_str(block);
            first = false;
            continue;
        }
        output.push_str("[[orchestration.nodes]]");
        if string_field(block, "id").as_deref() == Some(node_id) {
            let mut lines = block
                .lines()
                .filter(|line| !line.trim_start().starts_with("depends_on"))
                .map(str::to_string)
                .collect::<Vec<_>>();
            if !dependencies.is_empty() {
                lines.push(format!(
                    "depends_on = [{}]",
                    dependencies
                        .iter()
                        .map(|dependency| format!("\"{}\"", dependency.replace('"', "\\\"")))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            output.push_str(&lines.join("\n"));
            if block.ends_with('\n') {
                output.push('\n');
            }
        } else {
            output.push_str(block);
        }
    }
    output
}

fn input_handle(node: &WorkflowCanvasNode) -> egui::Pos2 {
    node.rect.left_center()
}

fn input_handle_at(node: &WorkflowCanvasNode, index: usize) -> egui::Pos2 {
    let slots = (node.inputs.len() + 1).max(1) as f32;
    egui::pos2(
        node.rect.left(),
        node.rect.top() + ((index as f32 + 1.0) * node.rect.height()) / (slots + 1.0),
    )
}

fn input_handles(node: &WorkflowCanvasNode) -> Vec<(usize, bool, egui::Pos2)> {
    if node.kind == CanvasNodeKind::Goal {
        return Vec::new();
    }
    (0..=node.inputs.len())
        .map(|index| {
            (
                index,
                index == node.inputs.len(),
                input_handle_at(node, index),
            )
        })
        .collect()
}

fn output_handle(node: &WorkflowCanvasNode) -> egui::Pos2 {
    node.rect.right_center()
}

fn output_handle_for_connection(node: &WorkflowCanvasNode, condition: Option<&str>) -> egui::Pos2 {
    if node.kind == CanvasNodeKind::Branch && condition == Some("true") {
        return egui::pos2(
            node.rect.right(),
            node.rect.top() + node.rect.height() * 0.34,
        );
    }
    if node.kind == CanvasNodeKind::Branch && condition == Some("false") {
        return egui::pos2(
            node.rect.right(),
            node.rect.top() + node.rect.height() * 0.66,
        );
    }
    output_handle(node)
}

fn output_handles(node: &WorkflowCanvasNode) -> Vec<(usize, Option<String>, egui::Pos2)> {
    if node.kind == CanvasNodeKind::Branch {
        vec![
            (
                0,
                Some("true".to_string()),
                output_handle_for_connection(node, Some("true")),
            ),
            (
                1,
                Some("false".to_string()),
                output_handle_for_connection(node, Some("false")),
            ),
        ]
    } else {
        vec![(0, None, output_handle(node))]
    }
}

fn find_connection_target(
    nodes: &[WorkflowCanvasNode],
    from_id: &str,
    pointer: egui::Pos2,
) -> Option<(String, Option<usize>)> {
    for node in nodes.iter().rev() {
        if node.kind == CanvasNodeKind::Goal || node.id == from_id {
            continue;
        }
        for (index, _, handle) in input_handles(node) {
            if handle.distance(pointer) <= 14.0 {
                return Some((node.id.clone(), Some(index)));
            }
        }
        if node.rect.contains(pointer)
            && output_handles(node)
                .iter()
                .all(|(_, _, handle)| handle.distance(pointer) > 14.0)
        {
            return Some((node.id.clone(), Some(node.inputs.len())));
        }
    }
    None
}

fn canvas_pointer_hits_scene(
    pointer: egui::Pos2,
    nodes: &[WorkflowCanvasNode],
    edges: &[(WorkflowCanvasConnection, egui::Pos2, egui::Pos2)],
) -> bool {
    nodes.iter().any(|node| {
        node.rect.contains(pointer)
            || input_handles(node)
                .iter()
                .any(|(_, _, handle)| handle.distance(pointer) <= 14.0)
            || output_handles(node)
                .iter()
                .any(|(_, _, handle)| handle.distance(pointer) <= 14.0)
    }) || edges
        .iter()
        .any(|(_, from, to)| distance_to_segment(pointer, *from, *to) <= 8.0)
}

fn draw_canvas_edge(
    painter: &egui::Painter,
    from: egui::Pos2,
    to: egui::Pos2,
    color: egui::Color32,
    width: f32,
) {
    painter.line_segment([from, to], egui::Stroke::new(width, color));
    painter.add(egui::Shape::convex_polygon(
        vec![to, to + egui::vec2(-8.0, -5.0), to + egui::vec2(-8.0, 5.0)],
        color,
        egui::Stroke::NONE,
    ));
}

fn draw_canvas_edge_label(painter: &egui::Painter, from: egui::Pos2, to: egui::Pos2, label: &str) {
    let center = from + (to - from) * 0.5;
    let text = if label == "true" { "True" } else { "False" };
    let rect = egui::Rect::from_center_size(center, egui::vec2(42.0, 20.0));
    painter.rect_filled(rect, 6.0, egui::Color32::from_rgb(254, 252, 232));
    painter.rect_stroke(
        rect,
        6.0,
        egui::Stroke::new(1.0, egui::Color32::from_rgb(250, 204, 21)),
    );
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(11.0),
        egui::Color32::from_rgb(133, 77, 14),
    );
}

fn distance_to_segment(point: egui::Pos2, start: egui::Pos2, end: egui::Pos2) -> f32 {
    let segment = end - start;
    let length_squared = segment.length_sq();
    if length_squared == 0.0 {
        return point.distance(start);
    }
    let projected = ((point - start).dot(segment) / length_squared).clamp(0.0, 1.0);
    point.distance(start + segment * projected)
}

fn draw_canvas_handle(
    painter: &egui::Painter,
    center: egui::Pos2,
    fill: egui::Color32,
    stroke: egui::Color32,
) {
    painter.circle_filled(center, 7.0, fill);
    painter.circle_stroke(center, 7.0, egui::Stroke::new(2.0, stroke));
}

fn draw_input_handles(painter: &egui::Painter, node: &WorkflowCanvasNode, stroke: egui::Color32) {
    for (index, is_append, center) in input_handles(node) {
        draw_canvas_handle(
            painter,
            center,
            if is_append {
                egui::Color32::from_rgb(248, 250, 252)
            } else {
                egui::Color32::WHITE
            },
            stroke,
        );
        painter.text(
            center,
            egui::Align2::CENTER_CENTER,
            if is_append {
                "+".to_string()
            } else {
                (index + 1).to_string()
            },
            egui::FontId::proportional(9.0),
            if is_append {
                egui::Color32::from_rgb(100, 116, 139)
            } else {
                egui::Color32::from_rgb(15, 23, 42)
            },
        );
    }
}

fn draw_output_handles(painter: &egui::Painter, node: &WorkflowCanvasNode) {
    for (_, condition, center) in output_handles(node) {
        draw_canvas_handle(
            painter,
            center,
            egui::Color32::from_rgb(14, 165, 233),
            egui::Color32::from_rgb(2, 132, 199),
        );
        if let Some(condition) = condition {
            painter.text(
                center,
                egui::Align2::CENTER_CENTER,
                if condition == "true" { "T" } else { "F" },
                egui::FontId::proportional(9.0),
                egui::Color32::WHITE,
            );
        }
    }
}

fn canvas_node_fill(kind: CanvasNodeKind, selected: bool) -> egui::Color32 {
    if selected {
        egui::Color32::from_rgb(224, 242, 254)
    } else {
        match kind {
            CanvasNodeKind::Goal => egui::Color32::from_rgb(254, 252, 232),
            CanvasNodeKind::Branch => egui::Color32::from_rgb(255, 251, 235),
            CanvasNodeKind::Agent => egui::Color32::WHITE,
        }
    }
}

fn canvas_node_stroke(kind: CanvasNodeKind, selected: bool) -> egui::Color32 {
    if selected {
        egui::Color32::from_rgb(2, 132, 199)
    } else {
        match kind {
            CanvasNodeKind::Goal => egui::Color32::from_rgb(234, 179, 8),
            CanvasNodeKind::Branch => egui::Color32::from_rgb(250, 204, 21),
            CanvasNodeKind::Agent => egui::Color32::from_rgb(186, 230, 253),
        }
    }
}

fn collect_files(root: PathBuf, files: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(path, files);
        } else {
            files.push(path.display().to_string());
        }
    }
}

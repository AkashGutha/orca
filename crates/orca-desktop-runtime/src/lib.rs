use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use orca_core::config::{default_orchestration_config, load_optional_config_with_settings};
use orca_core::goal::{GoalRequest, GoalSummary};
use orca_core::orchestrator::GoalOrchestrator;
use orca_core::output::{OutputEvent, OutputHandle};
use orca_core::planner::{PlanOptions, build_plan};
use orca_core::settings::{DEFAULT_SETTINGS_PATH, LoadedSettings, Settings};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::mpsc;

#[derive(Default)]
struct RunRegistry {
    handles: Mutex<HashMap<String, OutputHandle>>,
}

#[derive(Debug, Serialize)]
struct LoadedSettingsDto {
    settings: Settings,
    path: Option<String>,
}

#[derive(Debug, Serialize)]
struct WorkflowConfigDto {
    path: String,
    name: String,
    directory: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoalRunRequestDto {
    goal: String,
    config: Option<String>,
    settings: Settings,
    instruction_dir: Option<String>,
    artifact_dir: Option<String>,
    max_parallel_agents: Option<usize>,
    approve_golden_plan: bool,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct RunEventPayload {
    run_id: String,
    event: OutputEvent,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct RunErrorPayload {
    run_id: String,
    message: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct RunSummaryPayload {
    run_id: String,
    summary: GoalSummary,
}

pub fn run() {
    tauri::Builder::default()
        .manage(RunRegistry::default())
        .invoke_handler(tauri::generate_handler![
            load_settings,
            save_settings,
            list_workflow_configs,
            validate_workflow_config,
            load_workflow_config,
            save_workflow_config,
            start_goal_run,
            stop_goal_run,
            list_runs,
            list_artifacts,
            list_artifact_files,
            read_artifact,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run ORCA desktop app");
}

#[tauri::command]
fn load_settings(settings_path: Option<String>) -> Result<LoadedSettingsDto, String> {
    let loaded = match optional_path(settings_path.as_deref()) {
        Some(path) => Settings::load_optional(Some(&path)),
        None => Settings::load_default(),
    }
    .map_err(to_error_string)?;
    Ok(loaded_settings_dto(loaded))
}

#[tauri::command]
fn save_settings(settings_path: String, settings: Settings) -> Result<(), String> {
    let path =
        optional_path(Some(&settings_path)).unwrap_or_else(|| PathBuf::from(DEFAULT_SETTINGS_PATH));
    settings.save_to_path(&path).map_err(to_error_string)
}

#[tauri::command]
fn list_workflow_configs(settings: Settings) -> Vec<WorkflowConfigDto> {
    settings
        .workflow_configs()
        .into_iter()
        .map(workflow_config_dto)
        .collect()
}

#[tauri::command]
fn validate_workflow_config(path: Option<String>, settings: Settings) -> Result<(), String> {
    let config =
        load_optional_config_with_settings(optional_path(path.as_deref()).as_deref(), &settings)
            .map_err(to_error_string)?;
    if !config.commands.is_empty() {
        build_plan(config, PlanOptions::default()).map_err(to_error_string)?;
        return Ok(());
    }
    config
        .orchestration
        .unwrap_or_else(default_orchestration_config)
        .into_agent_config()
        .map_err(to_error_string)?;
    Ok(())
}

#[tauri::command]
fn load_workflow_config(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|source| format!("failed to read `{path}`: {source}"))
}

#[tauri::command]
fn save_workflow_config(path: String, content: String) -> Result<(), String> {
    if let Some(parent) = Path::new(&path).parent() {
        std::fs::create_dir_all(parent)
            .map_err(|source| format!("failed to create `{}`: {source}", parent.display()))?;
    }
    std::fs::write(&path, content).map_err(|source| format!("failed to write `{path}`: {source}"))
}

#[tauri::command]
async fn start_goal_run(
    app: AppHandle,
    registry: State<'_, RunRegistry>,
    request: GoalRunRequestDto,
) -> Result<String, String> {
    let goal = request.goal.trim().to_string();
    if goal.is_empty() {
        return Err("Enter a goal before starting a run.".to_string());
    }

    let run_id = new_run_id();
    let (sender, _receiver) = mpsc::unbounded_channel();
    let app_for_events = app.clone();
    let run_id_for_events = run_id.clone();
    let output_handle = OutputHandle::new(
        sender,
        Arc::new(std::sync::atomic::AtomicBool::new(false)),
        Some(Arc::new(move |event| {
            let _ = app_for_events.emit(
                "run:event",
                RunEventPayload {
                    run_id: run_id_for_events.clone(),
                    event,
                },
            );
        })),
    );

    registry
        .handles
        .lock()
        .map_err(|_| "run registry mutex poisoned".to_string())?
        .insert(run_id.clone(), output_handle.clone());

    let run_id_for_task = run_id.clone();
    let app_for_task = app.clone();
    tauri::async_runtime::spawn(async move {
        let goal_request = GoalRequest {
            goal,
            config: optional_path(request.config.as_deref()),
            settings: request.settings,
            instruction_dir: optional_path(request.instruction_dir.as_deref()),
            artifact_dir: optional_path(request.artifact_dir.as_deref()),
            max_parallel_agents: request.max_parallel_agents,
            approve_golden_plan: request.approve_golden_plan,
            json: false,
        };
        let result = GoalOrchestrator
            .run_with_output_handle(goal_request, Some(output_handle))
            .await;
        let registry_for_task = app_for_task.state::<RunRegistry>();
        if let Ok(mut handles) = registry_for_task.handles.lock() {
            handles.remove(&run_id_for_task);
        }
        match result {
            Ok(summary) => {
                let _ = app.emit(
                    "run:summary",
                    RunSummaryPayload {
                        run_id: run_id_for_task,
                        summary,
                    },
                );
            }
            Err(error) => {
                let _ = app.emit(
                    "run:error",
                    RunErrorPayload {
                        run_id: run_id_for_task,
                        message: error.to_string(),
                    },
                );
            }
        }
    });

    Ok(run_id)
}

#[tauri::command]
fn stop_goal_run(registry: State<'_, RunRegistry>, run_id: String) -> Result<(), String> {
    let handles = registry
        .handles
        .lock()
        .map_err(|_| "run registry mutex poisoned".to_string())?;
    let handle = handles
        .get(&run_id)
        .ok_or_else(|| format!("run `{run_id}` is not active"))?;
    handle.request_stop();
    Ok(())
}

#[tauri::command]
fn list_runs(settings: Settings) -> Result<Vec<String>, String> {
    artifact_roots(settings)
}

#[tauri::command]
fn list_artifacts(settings: Settings) -> Result<Vec<String>, String> {
    artifact_roots(settings)
}

#[tauri::command]
fn list_artifact_files(root: String) -> Vec<String> {
    let mut files = Vec::new();
    collect_files(Path::new(&root), &mut files);
    files.sort();
    files
}

#[tauri::command]
fn read_artifact(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|source| format!("failed to read `{path}`: {source}"))
}

fn artifact_roots(settings: Settings) -> Result<Vec<String>, String> {
    let artifact_dir = settings
        .default_artifact_dir()
        .unwrap_or_else(|| PathBuf::from("orca-runs"));
    let entries = std::fs::read_dir(&artifact_dir)
        .map_err(|source| format!("failed to read `{}`: {source}", artifact_dir.display()))?;
    let mut runs = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>();
    runs.sort();
    Ok(runs)
}

fn collect_files(dir: &Path, files: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, files);
        } else {
            files.push(path.display().to_string());
        }
    }
}

fn loaded_settings_dto(loaded: LoadedSettings) -> LoadedSettingsDto {
    LoadedSettingsDto {
        settings: loaded.settings,
        path: loaded.path.map(|path| path.display().to_string()),
    }
}

fn workflow_config_dto(path: PathBuf) -> WorkflowConfigDto {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("workflow")
        .to_string();
    let directory = path
        .parent()
        .map(|parent| parent.display().to_string())
        .unwrap_or_default();
    WorkflowConfigDto {
        path: path.display().to_string(),
        name,
        directory,
    }
}

fn optional_path(value: Option<&str>) -> Option<PathBuf> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn new_run_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("run-{millis}")
}

fn to_error_string(error: impl std::fmt::Display) -> String {
    error.to_string()
}

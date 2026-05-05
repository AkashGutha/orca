import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

import type {
  GoalRunRequest,
  LoadedSettings,
  RunErrorPayload,
  RunEventPayload,
  RunSummaryPayload,
  Settings,
  WorkflowConfig,
} from './types'

export function loadSettings(settingsPath?: string | null) {
  return invoke<LoadedSettings>('load_settings', { settingsPath })
}

export function saveSettings(settingsPath: string, settings: Settings) {
  return invoke<void>('save_settings', { settingsPath, settings })
}

export function listWorkflowConfigs(settings: Settings) {
  return invoke<WorkflowConfig[]>('list_workflow_configs', { settings })
}

export function validateWorkflowConfig(path: string | null, settings: Settings) {
  return invoke<void>('validate_workflow_config', { path, settings })
}

export function loadWorkflowConfig(path: string) {
  return invoke<string>('load_workflow_config', { path })
}

export function saveWorkflowConfig(path: string, content: string) {
  return invoke<void>('save_workflow_config', { path, content })
}

export function startGoalRun(request: GoalRunRequest) {
  return invoke<string>('start_goal_run', { request })
}

export function stopGoalRun(runId: string) {
  return invoke<void>('stop_goal_run', { runId })
}

export function listArtifacts(settings: Settings) {
  return invoke<string[]>('list_artifacts', { settings })
}

export function listArtifactFiles(root: string) {
  return invoke<string[]>('list_artifact_files', { root })
}

export function readArtifact(path: string) {
  return invoke<string>('read_artifact', { path })
}

export function onRunEvent(handler: (payload: RunEventPayload) => void) {
  return listen<RunEventPayload>('run:event', (event) => handler(event.payload))
}

export function onRunSummary(handler: (payload: RunSummaryPayload) => void) {
  return listen<RunSummaryPayload>('run:summary', (event) => handler(event.payload))
}

export function onRunError(handler: (payload: RunErrorPayload) => void) {
  return listen<RunErrorPayload>('run:error', (event) => handler(event.payload))
}

import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

import defaultWorkflowConfig from '../../../../config/orca.default.toml?raw'

import type {
  GoalRunRequest,
  LoadedSettings,
  RunErrorPayload,
  RunEventPayload,
  RunSummaryPayload,
  Settings,
  WorkflowConfig,
} from './types'

const previewSettings: Settings = {
  sources: {
    agents: ['agents'],
    instructions: ['instructions'],
    skills: ['skills'],
    workflows: ['config'],
  },
  defaults: {
    workflow: 'orca.default.toml',
    artifact_dir: 'orca-runs',
    max_parallel_agents: 8,
  },
}

const previewWorkflowConfigs: WorkflowConfig[] = [
  { path: 'config/orca.default.toml', name: 'orca.default.toml', directory: 'config' },
]

function hasTauriRuntime() {
  return typeof window !== 'undefined' && Boolean((globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__)
}

function unavailable(command: string) {
  return Promise.reject(new Error(`${command} is only available in the Tauri desktop runtime`))
}

export function loadSettings(settingsPath?: string | null) {
  if (!hasTauriRuntime()) return Promise.resolve({ settings: previewSettings, path: settingsPath ?? 'settings.toml' })
  return invoke<LoadedSettings>('load_settings', { settingsPath })
}

export function saveSettings(settingsPath: string, settings: Settings) {
  if (!hasTauriRuntime()) return Promise.resolve()
  return invoke<void>('save_settings', { settingsPath, settings })
}

export function listWorkflowConfigs(settings: Settings) {
  if (!hasTauriRuntime()) return Promise.resolve(previewWorkflowConfigs)
  return invoke<WorkflowConfig[]>('list_workflow_configs', { settings })
}

export function validateWorkflowConfig(path: string | null, settings: Settings) {
  if (!hasTauriRuntime()) return Promise.resolve()
  return invoke<void>('validate_workflow_config', { path, settings })
}

export function loadWorkflowConfig(path: string) {
  if (!hasTauriRuntime()) return Promise.resolve(defaultWorkflowConfig)
  return invoke<string>('load_workflow_config', { path })
}

export function saveWorkflowConfig(path: string, content: string) {
  if (!hasTauriRuntime()) return Promise.resolve()
  return invoke<void>('save_workflow_config', { path, content })
}

export function startGoalRun(request: GoalRunRequest) {
  if (!hasTauriRuntime()) return unavailable('start_goal_run')
  return invoke<string>('start_goal_run', { request })
}

export function stopGoalRun(runId: string) {
  if (!hasTauriRuntime()) return unavailable('stop_goal_run')
  return invoke<void>('stop_goal_run', { runId })
}

export function listArtifacts(settings: Settings) {
  if (!hasTauriRuntime()) return Promise.resolve(['orca-runs/preview-run'])
  return invoke<string[]>('list_artifacts', { settings })
}

export function listArtifactFiles(root: string) {
  if (!hasTauriRuntime()) return Promise.resolve([`${root}/manifest.json`, `${root}/planner/output.md`])
  return invoke<string[]>('list_artifact_files', { root })
}

export function readArtifact(path: string) {
  if (!hasTauriRuntime()) return Promise.resolve(`Preview artifact\n\n${path}`)
  return invoke<string>('read_artifact', { path })
}

export function browseDirectory() {
  if (!hasTauriRuntime()) return Promise.resolve(null)
  return invoke<string | null>('browse_directory')
}

export function onRunEvent(handler: (payload: RunEventPayload) => void) {
  if (!hasTauriRuntime()) return Promise.resolve(() => {})
  return listen<RunEventPayload>('run:event', (event) => handler(event.payload))
}

export function onRunSummary(handler: (payload: RunSummaryPayload) => void) {
  if (!hasTauriRuntime()) return Promise.resolve(() => {})
  return listen<RunSummaryPayload>('run:summary', (event) => handler(event.payload))
}

export function onRunError(handler: (payload: RunErrorPayload) => void) {
  if (!hasTauriRuntime()) return Promise.resolve(() => {})
  return listen<RunErrorPayload>('run:error', (event) => handler(event.payload))
}

export type SourceSettings = {
  agents: string[]
  instructions: string[]
  workflows: string[]
}

export type DefaultRunSettings = {
  workflow?: string | null
  artifact_dir?: string | null
  max_parallel_agents?: number | null
}

export type Settings = {
  sources: SourceSettings
  defaults: DefaultRunSettings
}

export type LoadedSettings = {
  settings: Settings
  path?: string | null
}

export type WorkflowConfig = {
  path: string
  name: string
  directory: string
}

export type GoalRunRequest = {
  goal: string
  config?: string | null
  settings: Settings
  instructionDir?: string | null
  artifactDir?: string | null
  maxParallelAgents?: number | null
  approveGoldenPlan: boolean
}

export type OutputEvent =
  | { PhaseStarted: { phase: string } }
  | { AgentStarted: { id: string; label: string; phase: string; model?: string | null } }
  | { AgentInput: { id: string; glimpse: string } }
  | { Line: { id: string; stream: 'Stdout' | 'Stderr'; line: string } }
  | { AgentFinished: { id: string; status: 'Running' | 'Succeeded' | 'Failed' } }
  | { IterationSummary: { iteration: number; summary: string; next_step: string } }
  | 'Shutdown'

export type RunEventPayload = {
  runId: string
  event: OutputEvent
}

export type RunSummary = {
  goal: string
  artifact_root: string
  approved: boolean
  completed: boolean
  iterations: number
  feedback?: string | null
  golden_plan_path?: string | null
  json: boolean
}

export type RunSummaryPayload = {
  runId: string
  summary: RunSummary
}

export type RunErrorPayload = {
  runId: string
  message: string
}

export type AgentPane = {
  id: string
  label: string
  phase: string
  model?: string | null
  status: 'Running' | 'Succeeded' | 'Failed'
  lines: string[]
}

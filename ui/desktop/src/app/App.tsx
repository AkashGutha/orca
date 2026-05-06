import { useEffect, useMemo, useRef, useState } from 'react'
import type { IconType } from 'react-icons'
import {
  PiArchiveDuotone as FolderArchive,
  PiArrowClockwiseDuotone as RefreshCw,
  PiCheckCircleDuotone,
  PiCodeDuotone,
  PiFileCodeDuotone as FileCode2,
  PiFilesDuotone,
  PiFileTextDuotone,
  PiFloppyDiskDuotone as Save,
  PiFlowArrowDuotone,
  PiFolderOpenDuotone,
  PiFolderPlusDuotone as FolderPlus,
  PiGearSixDuotone as SettingsIcon,
  PiGitBranchDuotone as GitBranchPlus,
  PiGraphDuotone,
  PiInfoDuotone,
  PiListChecksDuotone,
  PiPlayCircleDuotone,
  PiPlayDuotone as Play,
  PiPlusDuotone as Plus,
  PiRobotDuotone as Bot,
  PiSparkleDuotone,
  PiStopDuotone as Square,
  PiTargetDuotone,
  PiTrashDuotone as Trash2,
  PiUsersDuotone,
  PiXDuotone as X,
} from 'react-icons/pi'

import { Button } from '../components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card'
import { Input } from '../components/ui/input'
import { Textarea } from '../components/ui/textarea'
import {
  listArtifactFiles,
  listArtifacts,
  listWorkflowConfigs,
  loadWorkflowConfig,
  loadSettings,
  browseDirectory,
  onRunError,
  onRunEvent,
  onRunSummary,
  readArtifact,
  saveSettings,
  saveWorkflowConfig,
  startGoalRun,
  stopGoalRun,
  validateWorkflowConfig,
} from '../lib/tauri'
import type { AgentPane, OutputEvent, Settings, WorkflowConfig } from '../lib/types'

type AppPage = 'run' | 'workflows' | 'artifacts'

type WorkflowNode = {
  id: string
  kind: string
  evaluation: string
  output_contract: string
  instruction: string
  backend: string
  model: string
  depends_on: string[]
  inputs: string[]
}

type WorkflowConnection = {
  from: string
  to: string[]
  condition?: 'true' | 'false'
}

type WorkflowBackend = {
  id: string
  program: string
  args: string[]
}

type WorkflowDefaults = {
  backend: string
  model: string
}

type ParsedWorkflow = {
  nodes: WorkflowNode[]
  connections: WorkflowConnection[]
  backends: WorkflowBackend[]
  defaults: WorkflowDefaults
}

type Point = { x: number; y: number }

type Viewport = {
  pan: Point
  zoom: number
}

type CanvasNodeKind = 'goal' | 'agent' | 'branch'

type LayoutNode = {
  id: string
  label: string
  detail: string
  kind: CanvasNodeKind
  agentId?: string
  dependsOn?: string[]
  inputs?: string[]
  x: number
  y: number
  width: number
  height: number
}

type DragState = {
  ids: string[]
  offsets: Record<string, Point>
}

type PanState = {
  startPointer: Point
  startPan: Point
}

type ConnectionDragState = {
  fromId: string
  condition?: 'true' | 'false'
  pointer: Point
}

type OutputHandleHit = {
  node: LayoutNode
  condition?: 'true' | 'false'
}

type InputHandleHit = {
  node: LayoutNode
  index: number
  isAppend: boolean
}

type SelectedConnection = {
  fromId: string
  toId: string
  condition?: 'true' | 'false'
  inputIndex?: number
}

type CanvasEdge = SelectedConnection & {
  selectable: boolean
  offset?: number
}

const DEFAULT_CANVAS_VIEWPORT: Viewport = { pan: { x: 0, y: 0 }, zoom: 1 }
const MIN_CANVAS_ZOOM = 0.35
const MAX_CANVAS_ZOOM = 2.5

type CanvasHitTarget =
  | { kind: 'output'; nodeId: string; condition?: 'true' | 'false' }
  | { kind: 'input'; nodeId: string; index: number; isAppend: boolean }
  | { kind: 'node'; nodeId: string }
  | { kind: 'edge'; connection: SelectedConnection }
  | { kind: 'canvas' }

type MarqueeState = {
  start: Point
  current: Point
  additive: boolean
}

const fallbackSettings: Settings = {
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

const selectClassName =
  'h-8 w-full rounded-md border border-sky-200 bg-white px-2.5 text-xs font-medium text-slate-950 shadow-sm focus:border-orca-500 focus:outline-none focus:ring-1 focus:ring-orca-500'
const fieldLabelClassName = 'grid gap-1 text-xs font-medium text-slate-700'
const panelClassName = 'min-h-0 overflow-auto bg-white/44 p-3 backdrop-blur'
const iconClassName = 'mr-1.5 h-3.5 w-3.5'
const settingsInputClassName = 'h-7 px-2 text-[11px] shadow-none'

export function App() {
  const [settings, setSettings] = useState<Settings>(fallbackSettings)
  const [settingsPath, setSettingsPath] = useState('settings.toml')
  const [workflowConfigs, setWorkflowConfigs] = useState<WorkflowConfig[]>([])
  const [selectedWorkflow, setSelectedWorkflow] = useState('')
  const [goal, setGoal] = useState('')
  const [artifactDir, setArtifactDir] = useState('')
  const [instructionDir, setInstructionDir] = useState('')
  const [maxParallelAgents, setMaxParallelAgents] = useState('')
  const [approveGoldenPlan, setApproveGoldenPlan] = useState(false)
  const [activeRunId, setActiveRunId] = useState<string | null>(null)
  const [panes, setPanes] = useState<Record<string, AgentPane>>({})
  const [workflowContent, setWorkflowContent] = useState('')
  const [newWorkflowPath, setNewWorkflowPath] = useState('')
  const [artifactRoots, setArtifactRoots] = useState<string[]>([])
  const [selectedArtifact, setSelectedArtifact] = useState('')
  const [artifactFiles, setArtifactFiles] = useState<string[]>([])
  const [selectedArtifactFile, setSelectedArtifactFile] = useState('')
  const [artifactContent, setArtifactContent] = useState('')
  const [status, setStatus] = useState('Ready')
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [activePage, setActivePage] = useState<AppPage>('run')
  const paneList = useMemo(() => Object.values(panes), [panes])

  useEffect(() => {
    void loadInitialSettings()
  }, [])

  useEffect(() => {
    void refreshWorkflowConfigs(settings)
  }, [settings])

  useEffect(() => {
    const unlisteners = [
      onRunEvent((payload) => {
        if (payload.runId === activeRunId) {
          applyOutputEvent(payload.event)
        }
      }),
      onRunSummary((payload) => {
        if (payload.runId === activeRunId) {
          setStatus(
            payload.summary.completed
              ? `Completed in ${payload.summary.iterations} iteration(s)`
              : `Stopped after ${payload.summary.iterations} iteration(s)`,
          )
          setActiveRunId(null)
        }
      }),
      onRunError((payload) => {
        if (payload.runId === activeRunId) {
          setStatus(payload.message)
          setActiveRunId(null)
        }
      }),
    ]
    return () => {
      void Promise.all(unlisteners).then((callbacks) => callbacks.forEach((callback) => callback()))
    }
  }, [activeRunId])

  async function loadInitialSettings() {
    try {
      const loaded = await loadSettings()
      setSettings(loaded.settings)
      setSettingsPath(loaded.path ?? 'settings.toml')
      applySettingsDefaults(loaded.settings)
      setStatus('Settings loaded')
    } catch (error) {
      setStatus(String(error))
    }
  }

  async function refreshWorkflowConfigs(nextSettings = settings) {
    const configs = await listWorkflowConfigs(nextSettings)
    setWorkflowConfigs(configs)
    if (!selectedWorkflow) {
      const defaultWorkflow = resolveDefaultWorkflow(configs, nextSettings)
      setSelectedWorkflow(defaultWorkflow)
    }
  }

  function applySettingsDefaults(nextSettings: Settings) {
    setArtifactDir(nextSettings.defaults.artifact_dir ?? '')
    setMaxParallelAgents(nextSettings.defaults.max_parallel_agents?.toString() ?? '')
  }

  async function handleSaveSettings() {
    try {
      await saveSettings(settingsPath, settings)
      setStatus('Settings saved')
      applySettingsDefaults(settings)
      await refreshWorkflowConfigs(settings)
    } catch (error) {
      setStatus(String(error))
    }
  }

  async function handleValidateWorkflow() {
    try {
      await validateWorkflowConfig(selectedWorkflow || null, settings)
      setStatus('Workflow config is valid')
    } catch (error) {
      setStatus(String(error))
    }
  }

  async function handleLoadWorkflow() {
    if (!selectedWorkflow) {
      setStatus('Select a workflow config first')
      return
    }
    try {
      setWorkflowContent(await loadWorkflowConfig(selectedWorkflow))
      setStatus(`Loaded ${selectedWorkflow}`)
    } catch (error) {
      setStatus(String(error))
    }
  }

  async function handleSaveWorkflow() {
    if (!selectedWorkflow) {
      setStatus('Select a workflow config first')
      return
    }
    try {
      await saveWorkflowConfig(selectedWorkflow, workflowContent)
      setStatus(`Saved ${selectedWorkflow}`)
      await refreshWorkflowConfigs(settings)
    } catch (error) {
      setStatus(String(error))
    }
  }

  async function handleNewWorkflow() {
    const path = newWorkflowPath.trim()
    if (!path) {
      setStatus('Enter a new workflow path')
      return
    }
    const content =
      workflowContent ||
      `[orchestration]\nmax_parallel_agents = ${settings.defaults.max_parallel_agents ?? 8}\napproval_mode = "auto"\n\n`
    try {
      await saveWorkflowConfig(path, content)
      setSelectedWorkflow(path)
      setWorkflowContent(content)
      setStatus(`Created ${path}`)
      await refreshWorkflowConfigs(settings)
    } catch (error) {
      setStatus(String(error))
    }
  }

  async function handleDuplicateWorkflow() {
    const path = newWorkflowPath.trim()
    if (!path || !selectedWorkflow) {
      setStatus('Select a workflow and enter a duplicate path')
      return
    }
    try {
      const content = workflowContent || (await loadWorkflowConfig(selectedWorkflow))
      await saveWorkflowConfig(path, content)
      setSelectedWorkflow(path)
      setWorkflowContent(content)
      setStatus(`Duplicated workflow to ${path}`)
      await refreshWorkflowConfigs(settings)
    } catch (error) {
      setStatus(String(error))
    }
  }

  async function handleLoadArtifacts() {
    try {
      const roots = await listArtifacts(settings)
      setArtifactRoots(roots)
      if (!selectedArtifact) {
        setSelectedArtifact(roots[0] ?? '')
      }
      setStatus(`Loaded ${roots.length} artifact run(s)`)
    } catch (error) {
      setStatus(String(error))
    }
  }

  async function handleReadArtifact() {
    const path = selectedArtifactFile || selectedArtifact
    if (!path) {
      setStatus('Select an artifact file first')
      return
    }
    try {
      setArtifactContent(await readArtifact(path))
      setStatus(`Loaded ${path}`)
    } catch (error) {
      setStatus(String(error))
    }
  }

  async function handleListArtifactFiles() {
    if (!selectedArtifact) {
      setStatus('Select an artifact run first')
      return
    }
    try {
      const files = await listArtifactFiles(selectedArtifact)
      setArtifactFiles(files)
      setSelectedArtifactFile(files.find((file) => file.endsWith('manifest.json')) ?? files[0] ?? '')
      setStatus(`Loaded ${files.length} artifact file(s)`)
    } catch (error) {
      setStatus(String(error))
    }
  }

  async function handleStartRun() {
    try {
      setPanes({})
      const runId = await startGoalRun({
        goal,
        config: selectedWorkflow || null,
        settings,
        instructionDir: instructionDir || null,
        artifactDir: artifactDir || null,
        maxParallelAgents: maxParallelAgents ? Number(maxParallelAgents) : null,
        approveGoldenPlan,
      })
      setActiveRunId(runId)
      setStatus(`Run started: ${runId}`)
    } catch (error) {
      setStatus(String(error))
    }
  }

  async function handleStopRun() {
    if (!activeRunId) return
    try {
      await stopGoalRun(activeRunId)
      setStatus('Stop requested')
    } catch (error) {
      setStatus(String(error))
    }
  }

  function applyOutputEvent(event: OutputEvent) {
    if (typeof event === 'string') return
    if ('PhaseStarted' in event) {
      setPanes({})
      setStatus(`Phase: ${event.PhaseStarted.phase}`)
    } else if ('AgentStarted' in event) {
      const started = event.AgentStarted
      setPanes((current) => ({
        ...current,
        [started.id]: {
          id: started.id,
          label: started.label,
          phase: started.phase,
          model: started.model,
          status: 'Running',
          lines: [],
        },
      }))
    } else if ('Line' in event) {
      const line = event.Line
      setPanes((current) => {
        const pane = current[line.id]
        if (!pane) return current
        return {
          ...current,
          [line.id]: {
            ...pane,
            lines: [...pane.lines.slice(-300), line.line],
          },
        }
      })
    } else if ('AgentFinished' in event) {
      const finished = event.AgentFinished
      setPanes((current) => {
        const pane = current[finished.id]
        if (!pane) return current
        return {
          ...current,
          [finished.id]: { ...pane, status: finished.status },
        }
      })
    } else if ('IterationSummary' in event) {
      setStatus(`Iteration ${event.IterationSummary.iteration}: ${event.IterationSummary.next_step}`)
    }
  }

  return (
    <div className="flex h-full flex-col">
      <header className="flex h-12 items-center border-b border-sky-100 bg-white/84 px-4 shadow-sm shadow-sky-900/5 backdrop-blur">
        <div className="flex items-center gap-2.5">
          <div className="flex h-8 w-8 items-center justify-center rounded-md border border-orca-600 bg-orca-600 text-white shadow-sm shadow-sky-900/20">
            <Bot className="h-4 w-4" />
          </div>
          <div>
            <div className="text-sm font-semibold leading-4 text-slate-950">ORCA</div>
            <div className="text-[11px] leading-4 text-sky-700">Agents orchestration platform</div>
          </div>
        </div>
        <div className="ml-auto flex min-w-0 items-center gap-2">
          <div className="flex rounded-md border border-sky-200 bg-white p-0.5 shadow-sm">
            <PageButton page="run" activePage={activePage} setActivePage={setActivePage} label="Run" icon={PiPlayCircleDuotone} />
            <PageButton
              page="workflows"
              activePage={activePage}
              setActivePage={setActivePage}
              label="Workflows"
              icon={PiGraphDuotone}
            />
            <PageButton
              page="artifacts"
              activePage={activePage}
              setActivePage={setActivePage}
              label="Artifacts"
              icon={FolderArchive}
            />
          </div>
          <span className="max-w-sm truncate rounded-md border border-sky-100 bg-white/70 px-2 py-1 text-[11px] text-sky-800">
            {status}
          </span>
          <Button
            variant="outline"
            size="sm"
            aria-controls="settings-drawer"
            aria-expanded={settingsOpen}
            onClick={() => setSettingsOpen((open) => !open)}
          >
            <SettingsIcon className={iconClassName} />
            Settings
          </Button>
        </div>
      </header>

      {activePage === 'run' ? (
        <RunPage
          goal={goal}
          setGoal={setGoal}
          workflowConfigs={workflowConfigs}
          selectedWorkflow={selectedWorkflow}
          setSelectedWorkflow={setSelectedWorkflow}
          artifactDir={artifactDir}
          setArtifactDir={setArtifactDir}
          instructionDir={instructionDir}
          setInstructionDir={setInstructionDir}
          maxParallelAgents={maxParallelAgents}
          setMaxParallelAgents={setMaxParallelAgents}
          approveGoldenPlan={approveGoldenPlan}
          setApproveGoldenPlan={setApproveGoldenPlan}
          activeRunId={activeRunId}
          onStartRun={handleStartRun}
          onStopRun={handleStopRun}
          onValidateWorkflow={handleValidateWorkflow}
          paneList={paneList}
        />
      ) : null}

      {activePage === 'workflows' ? (
        <WorkflowPage
          workflowConfigs={workflowConfigs}
          selectedWorkflow={selectedWorkflow}
          setSelectedWorkflow={setSelectedWorkflow}
          workflowContent={workflowContent}
          setWorkflowContent={setWorkflowContent}
          newWorkflowPath={newWorkflowPath}
          setNewWorkflowPath={setNewWorkflowPath}
          onLoad={handleLoadWorkflow}
          onSave={handleSaveWorkflow}
          onCreate={handleNewWorkflow}
          onDuplicate={handleDuplicateWorkflow}
          onValidate={handleValidateWorkflow}
        />
      ) : null}

      {activePage === 'artifacts' ? (
        <ArtifactsPage
          artifactRoots={artifactRoots}
          selectedArtifact={selectedArtifact}
          artifactFiles={artifactFiles}
          selectedArtifactFile={selectedArtifactFile}
          artifactContent={artifactContent}
          setSelectedArtifact={setSelectedArtifact}
          setSelectedArtifactFile={setSelectedArtifactFile}
          onLoadArtifacts={handleLoadArtifacts}
          onListArtifactFiles={handleListArtifactFiles}
          onReadArtifact={handleReadArtifact}
        />
      ) : null}

      {settingsOpen ? (
        <>
          <button
            type="button"
            aria-label="Close settings"
            className="fixed inset-x-0 bottom-0 top-12 z-20 bg-slate-950/16 backdrop-blur-[1px]"
            onClick={() => setSettingsOpen(false)}
          />
          <aside
            id="settings-drawer"
            className="fixed bottom-0 right-0 top-12 z-30 w-[calc(100vw-1rem)] max-w-md border-l border-sky-100 bg-white/96 shadow-2xl shadow-sky-950/16 backdrop-blur"
          >
            <SettingsPanel
              settings={settings}
              settingsPath={settingsPath}
              setSettings={setSettings}
              setSettingsPath={setSettingsPath}
              onSave={handleSaveSettings}
              onReload={loadInitialSettings}
              onClose={() => setSettingsOpen(false)}
            />
          </aside>
        </>
      ) : null}
    </div>
  )
}

type PageButtonProps = {
  page: AppPage
  activePage: AppPage
  label: string
  icon: IconType
  setActivePage: (page: AppPage) => void
}

function PageButton({ page, activePage, label, icon: Icon, setActivePage }: PageButtonProps) {
  return (
    <Button
      variant={activePage === page ? 'secondary' : 'ghost'}
      size="sm"
      className="border-transparent shadow-none"
      onClick={() => setActivePage(page)}
    >
      <Icon className={iconClassName} />
      {label}
    </Button>
  )
}

type RunPageProps = {
  goal: string
  setGoal: (goal: string) => void
  workflowConfigs: WorkflowConfig[]
  selectedWorkflow: string
  setSelectedWorkflow: (path: string) => void
  artifactDir: string
  setArtifactDir: (path: string) => void
  instructionDir: string
  setInstructionDir: (path: string) => void
  maxParallelAgents: string
  setMaxParallelAgents: (value: string) => void
  approveGoldenPlan: boolean
  setApproveGoldenPlan: (value: boolean) => void
  activeRunId: string | null
  onStartRun: () => void
  onStopRun: () => void
  onValidateWorkflow: () => void
  paneList: AgentPane[]
}

function RunPage({
  goal,
  setGoal,
  workflowConfigs,
  selectedWorkflow,
  setSelectedWorkflow,
  artifactDir,
  setArtifactDir,
  instructionDir,
  setInstructionDir,
  maxParallelAgents,
  setMaxParallelAgents,
  approveGoldenPlan,
  setApproveGoldenPlan,
  activeRunId,
  onStartRun,
  onStopRun,
  onValidateWorkflow,
  paneList,
}: RunPageProps) {
  return (
    <main className="grid min-h-0 flex-1 grid-cols-[340px_1fr]">
      <aside className={`${panelClassName} border-r border-sky-100`}>
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <PiTargetDuotone className="h-4 w-4 text-orca-400" />
              Run goal
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <label className={fieldLabelClassName}>
              <span className="text-slate-700">Goal</span>
              <Textarea value={goal} onChange={(event) => setGoal(event.target.value)} />
            </label>
            <label className={fieldLabelClassName}>
              <span className="text-slate-700">Workflow config</span>
              <select
                className={selectClassName}
                value={selectedWorkflow}
                onChange={(event) => setSelectedWorkflow(event.target.value)}
              >
                <option value="">Use settings default</option>
                {workflowConfigs.map((config) => (
                  <option key={config.path} value={config.path}>
                    {config.name} ({config.directory})
                  </option>
                ))}
              </select>
            </label>
            <div className="grid grid-cols-2 gap-2">
              <label className={fieldLabelClassName}>
                <span className="text-slate-700">Artifact dir</span>
                <Input value={artifactDir} onChange={(event) => setArtifactDir(event.target.value)} />
              </label>
              <label className={fieldLabelClassName}>
                <span className="text-slate-700">Max agents</span>
                <Input value={maxParallelAgents} onChange={(event) => setMaxParallelAgents(event.target.value)} />
              </label>
            </div>
            <label className={fieldLabelClassName}>
              <span className="text-slate-700">Instruction override dir</span>
              <Input value={instructionDir} onChange={(event) => setInstructionDir(event.target.value)} />
            </label>
            <label className="flex items-center gap-2 rounded-md border border-sky-100 bg-white/70 px-2.5 py-2 text-xs font-medium text-slate-700">
              <input
                type="checkbox"
                checked={approveGoldenPlan}
                onChange={(event) => setApproveGoldenPlan(event.target.checked)}
              />
              Approve golden plan
            </label>
            <div className="flex gap-2">
              <Button className="flex-1" disabled={Boolean(activeRunId)} onClick={onStartRun}>
                <Play className={iconClassName} />
                Run
              </Button>
              <Button className="flex-1" variant="secondary" disabled={!activeRunId} onClick={onStopRun}>
                <Square className={iconClassName} />
                Stop
              </Button>
            </div>
            <Button variant="outline" className="w-full" onClick={onValidateWorkflow}>
              <PiCheckCircleDuotone className={iconClassName} />
              Validate workflow
            </Button>
          </CardContent>
        </Card>
      </aside>

      <section className="min-h-0 overflow-hidden p-3">
        <Card className="h-full min-h-0">
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <PiUsersDuotone className="h-4 w-4 text-orca-400" />
              Live agents
            </CardTitle>
          </CardHeader>
          <CardContent className="grid max-h-[calc(100vh-8rem)] gap-2 overflow-auto lg:grid-cols-2">
            {paneList.length === 0 ? (
              <div className="rounded-md border border-dashed border-sky-200 bg-white/50 p-6 text-center text-xs text-sky-700">
                <PiSparkleDuotone className="mx-auto mb-2 h-7 w-7 text-orca-400" />
                Start a run to stream agent output.
              </div>
            ) : (
              paneList.map((pane) => <AgentPaneView key={pane.id} pane={pane} />)
            )}
          </CardContent>
        </Card>
      </section>
    </main>
  )
}

function AgentPaneView({ pane }: { pane: AgentPane }) {
  return (
    <div className="min-h-64 rounded-md border border-sky-100 bg-white/86 shadow-sm">
      <div className="flex items-center justify-between border-b border-sky-100 px-3 py-2">
        <div>
          <div className="text-xs font-semibold text-slate-950">{pane.id}</div>
          <div className="text-xs text-sky-700">
            {pane.label} · {pane.model ?? 'default'}
          </div>
        </div>
        <span className="rounded-md border border-sky-200 bg-sky-50 px-2 py-1 text-[11px] font-medium text-sky-950">
          {pane.status}
        </span>
      </div>
      <pre className="max-h-80 overflow-auto whitespace-pre-wrap p-3 text-xs leading-5 text-slate-700">
        {pane.lines.join('\n')}
      </pre>
    </div>
  )
}

type WorkflowPageProps = {
  workflowConfigs: WorkflowConfig[]
  selectedWorkflow: string
  setSelectedWorkflow: (path: string) => void
  workflowContent: string
  newWorkflowPath: string
  setWorkflowContent: (content: string) => void
  setNewWorkflowPath: (path: string) => void
  onLoad: () => void
  onSave: () => void
  onCreate: () => void
  onDuplicate: () => void
  onValidate: () => void
}

function WorkflowPage({
  workflowConfigs,
  selectedWorkflow,
  setSelectedWorkflow,
  workflowContent,
  newWorkflowPath,
  setWorkflowContent,
  setNewWorkflowPath,
  onLoad,
  onSave,
  onCreate,
  onDuplicate,
  onValidate,
}: WorkflowPageProps) {
  const parsed = useMemo(() => parseWorkflow(workflowContent), [workflowContent])
  const [selectedNodeId, setSelectedNodeId] = useState('')
  const [selectedConnection, setSelectedConnection] = useState<SelectedConnection | null>(null)
  const [goalConnections, setGoalConnections] = useState<string[]>([])
  const [nodePositions, setNodePositions] = useState<Record<string, Point>>({})
  const selectedNode = parsed.nodes.find((node) => node.id === selectedNodeId) ?? parsed.nodes[0]

  useEffect(() => {
    if (parsed.nodes.length === 0) {
      setSelectedNodeId('')
    } else if (!parsed.nodes.some((node) => node.id === selectedNodeId)) {
      setSelectedNodeId(parsed.nodes[0].id)
    }
  }, [parsed.nodes, selectedNodeId])

  useEffect(() => {
    const ids = new Set(parsed.nodes.map((node) => node.id))
    const goalInputIds = parsed.nodes.filter((node) => node.inputs.includes('goal')).map((node) => node.id)
    setGoalConnections((connections) => Array.from(new Set([...connections.filter((connection) => ids.has(connection)), ...goalInputIds])))
  }, [parsed.nodes])

  function setParsed(nextNodes: WorkflowNode[], nextConnections = parsed.connections) {
    setWorkflowContent(serializeWorkflow(nextNodes, nextConnections, parsed.defaults, parsed.backends))
  }

  function addNode() {
    const id = uniqueNodeId(parsed.nodes, 'agent')
    setSelectedNodeId(id)
    setSelectedConnection(null)
    setNodePositions((positions) => ({ ...positions, [id]: positions[id] ?? defaultAgentPosition(parsed.nodes.length) }))
    setParsed([
      ...parsed.nodes,
      {
        id,
        kind: 'agent',
        evaluation: '',
        output_contract: 'implementation',
        instruction: 'work.md',
        backend: '',
        model: '',
        depends_on: [],
        inputs: [],
      },
    ])
  }

  function addBranchNode() {
    const id = uniqueNodeId(parsed.nodes, 'branch')
    setSelectedNodeId(id)
    setSelectedConnection(null)
    setNodePositions((positions) => ({ ...positions, [id]: positions[id] ?? defaultAgentPosition(parsed.nodes.length) }))
    setParsed([
      ...parsed.nodes,
      {
        id,
        kind: 'branch',
        evaluation: 'input contains signed_off = true',
        output_contract: 'branch',
        instruction: '',
        backend: '',
        model: '',
        depends_on: [],
        inputs: [],
      },
    ])
  }

  function updateNode(updated: WorkflowNode) {
    const previousId = selectedNode?.id ?? updated.id
    if (previousId !== updated.id) {
      setNodePositions((positions) => renamePosition(positions, previousId, updated.id))
    }
    setParsed(
      parsed.nodes.map((node) =>
        node.id === previousId
          ? updated
          : {
              ...node,
              depends_on: node.depends_on.map((dependency) => (dependency === previousId ? updated.id : dependency)),
              inputs: node.inputs.map((source) => (source === previousId ? updated.id : source)),
            },
      ),
      parsed.connections.map((connection) => ({
        ...connection,
        from: connection.from === previousId ? updated.id : connection.from,
        to: connection.to.map((target) => (target === previousId ? updated.id : target)),
      })),
    )
    setSelectedNodeId(updated.id)
  }

  function removeNode(id: string) {
    removeNodes([id])
  }

  function removeNodes(ids: string[]) {
    const removeIds = new Set(ids)
    setNodePositions((positions) =>
      Object.fromEntries(Object.entries(positions).filter(([nodeId]) => !removeIds.has(nodeId))),
    )
    setSelectedConnection(null)
    setGoalConnections((connections) => connections.filter((connection) => !removeIds.has(connection)))
    setParsed(
      parsed.nodes
        .filter((node) => !removeIds.has(node.id))
        .map((node) => ({
          ...node,
          depends_on: node.depends_on.filter((dependency) => !removeIds.has(dependency)),
          inputs: node.inputs.filter((source) => !removeIds.has(source)),
        })),
      parsed.connections
        .filter((connection) => !removeIds.has(connection.from))
        .map((connection) => ({ ...connection, to: connection.to.filter((target) => !removeIds.has(target)) }))
        .filter((connection) => connection.to.length > 0),
    )
  }

  function addConnection(from: string, to: string, requestedCondition?: 'true' | 'false', inputIndex?: number) {
    if (!from || !to || from === to) return undefined
    const fromNode = parsed.nodes.find((node) => node.id === from)
    if (fromNode?.kind === 'branch') {
      const condition = requestedCondition ?? nextBranchCondition(parsed.connections, from)
      const nextConnections = upsertWorkflowConnection(parsed.connections, from, to, condition)
      setParsed(parsed.nodes, nextConnections)
      return condition
    }
    const nextNodes = parsed.nodes.map((node) =>
      node.id === to
        ? withOrderedInput(node, from, inputIndex)
        : node,
    )
    setParsed(nextNodes)
    return undefined
  }

  function removeConnection(from: string, to: string) {
    setSelectedConnection(null)
    if (from === GOAL_NODE_ID) {
      setGoalConnections((connections) => connections.filter((connection) => connection !== to))
      setParsed(parsed.nodes.map((node) => (node.id === to ? withoutOrderedInput(node, 'goal') : node)))
      return
    }
    setParsed(
      parsed.nodes.map((node) =>
        node.id === to ? withoutOrderedInput(node, from) : node,
      ),
      parsed.connections
        .map((connection) =>
          connection.from === from && connection.condition === selectedConnection?.condition
            ? { ...connection, to: connection.to.filter((target) => target !== to) }
            : connection,
        )
        .filter((connection) => connection.to.length > 0),
    )
  }

  function moveCanvasNode(id: string, position: Point) {
    setNodePositions((positions) => ({ ...positions, [id]: position }))
  }

  function connectCanvasNodes(fromId: string, toId: string, condition?: 'true' | 'false', inputIndex?: number) {
    const from = resolveCanvasEndpoint(fromId)
    const to = resolveCanvasEndpoint(toId)
    if (!from || !to || to.kind === 'goal') return
    if (from.kind === 'goal') {
      setParsed(parsed.nodes.map((node) => (node.id === to.agentId ? withOrderedInput({ ...node, depends_on: [] }, 'goal', inputIndex) : node)))
      setSelectedNodeId(to.agentId)
      setGoalConnections((connections) => (connections.includes(to.agentId) ? connections : [...connections, to.agentId]))
      setSelectedConnection({ fromId: GOAL_NODE_ID, toId: to.agentId, inputIndex })
      return
    }
    if (from.agentId !== to.agentId) {
      setGoalConnections((connections) => connections.filter((connection) => connection !== to.agentId))
      const connectionCondition = addConnection(from.agentId, to.agentId, condition, inputIndex)
      setSelectedNodeId(to.agentId)
      setSelectedConnection({ fromId: from.agentId, toId: to.agentId, condition: connectionCondition, inputIndex })
    }
  }

  function handleSelectNode(id: string) {
    setSelectedNodeId(id)
    setSelectedConnection(null)
  }

  function handleDeleteSelection() {
    if (selectedConnection) {
      removeConnection(selectedConnection.fromId, selectedConnection.toId)
      return
    }
    if (selectedNodeId) {
      removeNode(selectedNodeId)
    }
  }

  return (
    <main className="grid min-h-0 flex-1 grid-cols-[292px_1fr_340px]">
      <aside className={`${panelClassName} border-r border-sky-100`}>
        <div className="space-y-3">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <FileCode2 className="h-4 w-4 text-orca-400" />
                Workflows
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-3">
              <select
                className={selectClassName}
                value={selectedWorkflow}
                onChange={(event) => setSelectedWorkflow(event.target.value)}
              >
                <option value="">Select workflow</option>
                {workflowConfigs.map((config) => (
                  <option key={config.path} value={config.path}>
                    {config.name}
                  </option>
                ))}
              </select>
              <div className="grid grid-cols-2 gap-2">
                <Button variant="secondary" onClick={onLoad}>
                  <RefreshCw className={iconClassName} />
                  Load
                </Button>
                <Button onClick={onSave}>
                  <Save className={iconClassName} />
                  Save
                </Button>
                <Button variant="secondary" onClick={onCreate}>
                  <Plus className={iconClassName} />
                  New
                </Button>
                <Button variant="outline" onClick={onValidate}>
                  <PiCheckCircleDuotone className={iconClassName} />
                  Validate
                </Button>
              </div>
              <label className={fieldLabelClassName}>
                <span className="text-slate-700">New or duplicate path</span>
                <Input
                  value={newWorkflowPath}
                  onChange={(event) => setNewWorkflowPath(event.target.value)}
                  placeholder="config/new-workflow.toml"
                />
              </label>
              <Button className="w-full" variant="secondary" onClick={onDuplicate}>
                <PiFilesDuotone className={iconClassName} />
                Duplicate selected workflow
              </Button>
            </CardContent>
          </Card>
          <ConnectionBuilder nodes={parsed.nodes} onAddConnection={addConnection} />
        </div>
      </aside>

      <section className="min-h-0 overflow-hidden p-3">
        <WorkflowCanvas
          nodes={parsed.nodes}
          connections={parsed.connections}
          goalConnections={goalConnections}
          nodePositions={nodePositions}
          selectedNodeId={selectedNode?.id ?? ''}
          selectedConnection={selectedConnection}
          onSelectNode={handleSelectNode}
          onSelectConnection={setSelectedConnection}
          onMoveNode={moveCanvasNode}
          onConnectNodes={connectCanvasNodes}
          onDeleteSelection={handleDeleteSelection}
          onDeleteNodes={removeNodes}
        />
      </section>

      <aside className={`${panelClassName} border-l border-sky-100`}>
        <NodeInspector
          node={selectedNode}
          nodes={parsed.nodes}
          backendOptions={workflowBackendOptions(parsed)}
          modelOptions={workflowModelOptions(parsed)}
          defaultBackend={parsed.defaults.backend || 'copilot'}
          defaultModel={parsed.defaults.model || 'default'}
          onAddNode={addNode}
          onAddBranchNode={addBranchNode}
          onUpdateNode={updateNode}
          onRemoveNode={removeNode}
        />
        <Card className="mt-3">
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <PiCodeDuotone className="h-4 w-4 text-orca-400" />
              Workflow source
            </CardTitle>
          </CardHeader>
          <CardContent>
            <Textarea
              className="min-h-64 font-mono text-[11px] leading-5"
              value={workflowContent}
              onChange={(event) => setWorkflowContent(event.target.value)}
              placeholder="Load a workflow config or add a node to create one."
            />
          </CardContent>
        </Card>
      </aside>
    </main>
  )

  function resolveCanvasEndpoint(id: string): { kind: CanvasNodeKind; agentId: string } | null {
    if (id === GOAL_NODE_ID) return { kind: 'goal', agentId: id }
    const node = parsed.nodes.find((candidate) => candidate.id === id)
    if (node) return { kind: node.kind === 'branch' ? 'branch' : 'agent', agentId: id }
    return null
  }
}

type WorkflowCanvasProps = {
  nodes: WorkflowNode[]
  connections: WorkflowConnection[]
  goalConnections: string[]
  nodePositions: Record<string, Point>
  selectedNodeId: string
  selectedConnection: SelectedConnection | null
  onSelectNode: (id: string) => void
  onSelectConnection: (connection: SelectedConnection | null) => void
  onMoveNode: (id: string, position: Point) => void
  onConnectNodes: (fromId: string, toId: string, condition?: 'true' | 'false', inputIndex?: number) => void
  onDeleteSelection: () => void
  onDeleteNodes: (ids: string[]) => void
}

function WorkflowCanvas({
  nodes,
  connections,
  goalConnections,
  nodePositions,
  selectedNodeId,
  selectedConnection,
  onSelectNode,
  onSelectConnection,
  onMoveNode,
  onConnectNodes,
  onDeleteSelection,
  onDeleteNodes,
}: WorkflowCanvasProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null)
  const helpButtonRef = useRef<HTMLButtonElement | null>(null)
  const panningRef = useRef<PanState | null>(null)
  const [dragging, setDragging] = useState<DragState | null>(null)
  const [connecting, setConnecting] = useState<ConnectionDragState | null>(null)
  const [panning, setPanning] = useState<PanState | null>(null)
  const [viewport, setViewport] = useState<Viewport>(DEFAULT_CANVAS_VIEWPORT)
  const [selectedNodeIds, setSelectedNodeIds] = useState<string[]>(selectedNodeId ? [selectedNodeId] : [])
  const [hoverTarget, setHoverTarget] = useState<CanvasHitTarget | null>(null)
  const [marquee, setMarquee] = useState<MarqueeState | null>(null)
  const [spacePressed, setSpacePressed] = useState(false)
  const [helpOpen, setHelpOpen] = useState(false)
  const layout = useMemo(() => layoutWorkflowCanvas(nodes, nodePositions), [nodes, nodePositions])

  useEffect(() => {
    const nodeIds = new Set(nodes.map((node) => node.id))
    setSelectedNodeIds((ids) => {
      const kept = ids.filter((id) => nodeIds.has(id))
      if (selectedNodeId && !kept.includes(selectedNodeId)) return [selectedNodeId]
      if (!selectedNodeId) return []
      return kept.length > 0 ? kept : [selectedNodeId]
    })
  }, [nodes, selectedNodeId])

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const context = canvas.getContext('2d')
    if (!context) return
    const ratio = window.devicePixelRatio || 1
    const rect = canvas.getBoundingClientRect()
    canvas.width = Math.max(1, Math.floor(rect.width * ratio))
    canvas.height = Math.max(1, Math.floor(rect.height * ratio))
    context.setTransform(ratio, 0, 0, ratio, 0, 0)
    drawWorkflowCanvas(
      context,
      rect.width,
      rect.height,
      layout,
      connections,
      goalConnections,
      selectedNodeIds,
      selectedConnection,
      connecting,
      hoverTarget,
      marquee,
      viewport,
    )
  }, [connections, connecting, goalConnections, hoverTarget, layout, marquee, selectedConnection, selectedNodeIds, viewport])

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const targetCanvas = canvas
    function handleWheel(event: WheelEvent) {
      if (!event.ctrlKey && !event.metaKey) return
      event.preventDefault()
      event.stopPropagation()
      const rect = targetCanvas.getBoundingClientRect()
      const anchor = { x: event.clientX - rect.left, y: event.clientY - rect.top }
      setViewport((current) => zoomViewport(current, anchor, event.deltaY))
    }
    targetCanvas.addEventListener('wheel', handleWheel, { passive: false })
    return () => targetCanvas.removeEventListener('wheel', handleWheel)
  }, [])

  function eventPoints(event: React.PointerEvent<HTMLCanvasElement>) {
    const rect = event.currentTarget.getBoundingClientRect()
    const screen = { x: event.clientX - rect.left, y: event.clientY - rect.top }
    return { screen, world: screenToWorld(screen, viewport) }
  }

  function selectSingleNode(id: string) {
    setSelectedNodeIds([id])
    onSelectNode(id)
    onSelectConnection(null)
  }

  function toggleNodeSelection(id: string) {
    const next = selectedNodeIds.includes(id)
      ? selectedNodeIds.filter((nodeId) => nodeId !== id)
      : [...selectedNodeIds, id]
    setSelectedNodeIds(next)
    if (next.length > 0) onSelectNode(id)
    else onSelectConnection(null)
    return next
  }

  function startDraggingNodes(ids: string[], world: Point) {
    const offsets = Object.fromEntries(
      ids
        .map((id) => layout.find((node) => node.id === id))
        .filter((node): node is LayoutNode => Boolean(node))
        .map((node) => [node.id, { x: world.x - node.x, y: world.y - node.y }]),
    )
    const draggableIds = Object.keys(offsets)
    if (draggableIds.length > 0) setDragging({ ids: draggableIds, offsets })
  }

  function handlePointerDown(event: React.PointerEvent<HTMLCanvasElement>) {
    event.currentTarget.setPointerCapture(event.pointerId)
    event.currentTarget.focus()
    const { screen, world } = eventPoints(event)
    const target = canvasHitTarget(layout, connections, goalConnections, world.x, world.y)
    setHoverTarget(target)
    const shouldPan = event.button === 1 || event.button === 2 || spacePressed
    if (target.kind === 'output') {
      onSelectConnection(null)
      setSelectedNodeIds([])
      setConnecting({ fromId: target.nodeId, condition: target.condition, pointer: world })
      return
    }
    if (target.kind === 'node') {
      const hit = layout.find((node) => node.id === target.nodeId)
      if (!hit) return
      if (hit.kind !== 'goal') {
        const nextSelection = event.shiftKey ? toggleNodeSelection(hit.id) : selectedNodeIds.includes(hit.id) ? selectedNodeIds : [hit.id]
        if (!event.shiftKey && !selectedNodeIds.includes(hit.id)) selectSingleNode(hit.id)
        if (!event.shiftKey && selectedNodeIds.includes(hit.id)) {
          onSelectNode(hit.id)
          onSelectConnection(null)
        }
        startDraggingNodes(nextSelection.includes(hit.id) ? nextSelection : [hit.id], world)
      }
      else onSelectConnection(null)
      return
    }
    if (target.kind === 'edge') {
      setSelectedNodeIds([])
      onSelectConnection(target.connection)
    } else {
      if (!event.shiftKey) {
        setSelectedNodeIds([])
        onSelectConnection(null)
      }
      if (shouldPan) {
        const nextPanning = { startPointer: screen, startPan: viewport.pan }
        panningRef.current = nextPanning
        setPanning(nextPanning)
      } else {
        setMarquee({ start: world, current: world, additive: event.shiftKey })
      }
    }
  }

  function handlePointerMove(event: React.PointerEvent<HTMLCanvasElement>) {
    const { screen, world } = eventPoints(event)
    setHoverTarget(canvasHitTarget(layout, connections, goalConnections, world.x, world.y))
    if (connecting) {
      setConnecting({ ...connecting, pointer: world })
      return
    }
    if (dragging) {
      for (const id of dragging.ids) {
        const offset = dragging.offsets[id]
        if (offset) onMoveNode(id, { x: world.x - offset.x, y: world.y - offset.y })
      }
      return
    }
    if (marquee) {
      setMarquee({ ...marquee, current: world })
      return
    }
    const activePanning = panningRef.current ?? panning
    if (activePanning) {
      setViewport((current) => ({
        ...current,
        pan: {
          x: activePanning.startPan.x + screen.x - activePanning.startPointer.x,
          y: activePanning.startPan.y + screen.y - activePanning.startPointer.y,
        },
      }))
    }
  }

  function handlePointerUp(event: React.PointerEvent<HTMLCanvasElement>) {
    const { world } = eventPoints(event)
    if (connecting) {
      const target = findConnectionTarget(layout, connecting.fromId, world.x, world.y)
      if (target) onConnectNodes(connecting.fromId, target.node.id, connecting.condition, target.index)
    }
    if (marquee) {
      const rect = normalizedRect(marquee.start, marquee.current)
      const selected = layout
        .filter((node) => node.kind !== 'goal' && rectIntersectsNode(rect, node))
        .map((node) => node.id)
      const next = marquee.additive ? Array.from(new Set([...selectedNodeIds, ...selected])) : selected
      setSelectedNodeIds(next)
      if (next.length > 0) onSelectNode(next[next.length - 1])
      onSelectConnection(null)
    }
    setDragging(null)
    setConnecting(null)
    setMarquee(null)
    panningRef.current = null
    setPanning(null)
  }

  function clearCanvasGesture() {
    setDragging(null)
    setConnecting(null)
    setMarquee(null)
    panningRef.current = null
    setPanning(null)
  }

  function handleKeyDown(event: React.KeyboardEvent<HTMLCanvasElement>) {
    if (event.key === ' ') setSpacePressed(true)
    if (event.key === 'Delete' || event.key === 'Backspace') {
      event.preventDefault()
      if (selectedNodeIds.length > 1) {
        onDeleteNodes(selectedNodeIds)
        setSelectedNodeIds([])
      } else {
        onDeleteSelection()
      }
    }
    if (event.key === 'Escape') {
      event.preventDefault()
      clearCanvasGesture()
      setSelectedNodeIds([])
      onSelectConnection(null)
    }
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'a') {
      event.preventDefault()
      const ids = layout.filter((node) => node.kind !== 'goal').map((node) => node.id)
      setSelectedNodeIds(ids)
      if (ids[0]) onSelectNode(ids[0])
      onSelectConnection(null)
    }
    if ((event.metaKey || event.ctrlKey) && event.key === '0') {
      event.preventDefault()
      setViewport(DEFAULT_CANVAS_VIEWPORT)
    }
    if (['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(event.key) && selectedNodeIds.length > 0) {
      event.preventDefault()
      const amount = event.shiftKey ? 24 : 8
      const delta = {
        x: event.key === 'ArrowLeft' ? -amount : event.key === 'ArrowRight' ? amount : 0,
        y: event.key === 'ArrowUp' ? -amount : event.key === 'ArrowDown' ? amount : 0,
      }
      for (const id of selectedNodeIds) {
        const node = layout.find((candidate) => candidate.id === id)
        if (node) onMoveNode(id, { x: node.x + delta.x, y: node.y + delta.y })
      }
    }
    if (event.key === '?') {
      event.preventDefault()
      setHelpOpen(true)
    }
  }

  function handleKeyUp(event: React.KeyboardEvent<HTMLCanvasElement>) {
    if (event.key === ' ') setSpacePressed(false)
  }

  const cursorClassName = canvasCursorClassName({
    connecting,
    dragging,
    panning: Boolean(panningRef.current ?? panning),
    marquee,
    hoverTarget,
    connectionTarget: connecting ? findConnectionTarget(layout, connecting.fromId, connecting.pointer.x, connecting.pointer.y) : null,
    spacePressed,
  })

  const selectedCount = selectedNodeIds.length

  return (
    <Card className="h-full min-h-0 overflow-hidden">
      <CardHeader>
        <div className="flex items-center justify-between gap-3">
          <CardTitle className="flex items-center gap-2">
            <PiFlowArrowDuotone className="h-4 w-4 text-orca-400" />
            Workflow canvas
          </CardTitle>
          <div className="flex items-center gap-2">
            {selectedCount > 1 ? (
              <span className="rounded-md border border-sky-100 bg-sky-50 px-2 py-0.5 text-[11px] font-medium text-sky-950">
                {selectedCount} selected
              </span>
            ) : null}
            <KeyboardLegend />
            <Button
              ref={helpButtonRef}
              variant="outline"
              size="sm"
              className="px-2"
              aria-label="Canvas features"
              onClick={() => setHelpOpen(true)}
            >
              <PiInfoDuotone className="h-4 w-4" />
            </Button>
          </div>
        </div>
      </CardHeader>
      <CardContent className="h-[calc(100%-5rem)] min-h-0">
        <canvas
          ref={canvasRef}
          tabIndex={0}
          className={`h-full w-full rounded-md border border-sky-100 bg-white touch-none focus:outline-none focus:ring-2 focus:ring-orca-400 ${cursorClassName}`}
          onContextMenu={(event) => event.preventDefault()}
          onKeyDown={handleKeyDown}
          onKeyUp={handleKeyUp}
          onPointerDown={handlePointerDown}
          onPointerMove={handlePointerMove}
          onPointerUp={handlePointerUp}
          onPointerLeave={() => setHoverTarget(null)}
          onPointerCancel={() => {
            clearCanvasGesture()
          }}
        />
      </CardContent>
      {helpOpen ? <CanvasHelpModal onClose={() => {
        setHelpOpen(false)
        helpButtonRef.current?.focus()
      }} /> : null}
    </Card>
  )
}

function KeyboardLegend() {
  return (
    <div className="flex flex-wrap justify-end gap-1.5 text-[11px] text-slate-500">
      <span className="rounded-md border border-sky-100 bg-white/80 px-2 py-0.5">Shift select</span>
      <span className="rounded-md border border-sky-100 bg-white/80 px-2 py-0.5">Drag box</span>
      <span className="rounded-md border border-sky-100 bg-white/80 px-2 py-0.5">Space pan</span>
      <span className="rounded-md border border-sky-100 bg-white/80 px-2 py-0.5">Ctrl scroll zoom</span>
      <span className="rounded-md border border-sky-100 bg-white/80 px-2 py-0.5">? help</span>
    </div>
  )
}

function CanvasHelpModal({ onClose }: { onClose: () => void }) {
  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === 'Escape') onClose()
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [onClose])

  return (
    <>
      <button
        type="button"
        aria-label="Close canvas features"
        className="fixed inset-0 z-40 bg-slate-950/18 backdrop-blur-[1px]"
        onClick={onClose}
      />
      <section
        role="dialog"
        aria-modal="true"
        aria-labelledby="canvas-help-title"
        className="fixed left-1/2 top-1/2 z-50 flex max-h-[calc(100vh-3rem)] w-[min(680px,calc(100vw-2rem))] -translate-x-1/2 -translate-y-1/2 flex-col rounded-md border border-sky-100 bg-white shadow-2xl shadow-sky-950/20"
      >
        <div className="flex items-center justify-between gap-3 border-b border-sky-100 px-3 py-2.5">
          <h2 id="canvas-help-title" className="flex items-center gap-2 text-sm font-semibold text-slate-950">
            <PiInfoDuotone className="h-4 w-4 text-orca-400" />
            Canvas features
          </h2>
          <Button variant="ghost" size="sm" className="px-2" aria-label="Close canvas features" onClick={onClose}>
            <X className="h-4 w-4" />
          </Button>
        </div>
        <div className="min-h-0 space-y-4 overflow-auto p-4 text-xs leading-5 text-slate-700">
          <CanvasHelpSection
            title="Selection"
            items={[
              'Click a node to inspect it in the right panel.',
              'Shift-click nodes to build or trim a multi-selection.',
              'Drag on empty canvas to select nodes with a box.',
              'Click an edge to select it for deletion.',
            ]}
          />
          <CanvasHelpSection
            title="Moving"
            items={[
              'Drag any selected node to move the whole selection.',
              'Use arrow keys to nudge selected nodes.',
              'Hold Shift with arrow keys for larger nudges.',
              'Press Ctrl+A or Cmd+A while the canvas is focused to select all nodes.',
            ]}
          />
          <CanvasHelpSection
            title="Canvas"
            items={[
              'Hold Space and drag, middle-drag, or right-drag to pan.',
              'Use Ctrl+scroll or Cmd+scroll over the canvas to zoom only the workflow view.',
              'Press Ctrl+0 or Cmd+0 while the canvas is focused to reset pan and zoom.',
              'Press Escape to cancel an active drag or clear canvas selection.',
              'Cursor changes show when you can move, pan, connect, select, or cannot connect.',
            ]}
          />
          <CanvasHelpSection
            title="Connections"
            items={[
              'Drag from an output circle to another node input to create a dependency.',
              'Branch nodes expose True and False output handles.',
              'Dense graphs fade unrelated connections while you hover or select a node.',
              'Selected and hovered connections draw above the rest for easier targeting.',
            ]}
          />
        </div>
      </section>
    </>
  )
}

function CanvasHelpSection({ title, items }: { title: string; items: string[] }) {
  return (
    <section>
      <h3 className="mb-1 text-xs font-semibold uppercase tracking-wide text-sky-800">{title}</h3>
      <ul className="grid gap-1">
        {items.map((item) => (
          <li key={item} className="rounded-md border border-sky-100 bg-sky-50/50 px-2.5 py-1.5">
            {item}
          </li>
        ))}
      </ul>
    </section>
  )
}

type NodeInspectorProps = {
  node?: WorkflowNode
  nodes: WorkflowNode[]
  backendOptions: string[]
  modelOptions: string[]
  defaultBackend: string
  defaultModel: string
  onAddNode: () => void
  onAddBranchNode: () => void
  onUpdateNode: (node: WorkflowNode) => void
  onRemoveNode: (id: string) => void
}

function NodeInspector({
  node,
  nodes,
  backendOptions,
  modelOptions,
  defaultBackend,
  defaultModel,
  onAddNode,
  onAddBranchNode,
  onUpdateNode,
  onRemoveNode,
}: NodeInspectorProps) {
  if (!node) {
    return (
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <PiListChecksDuotone className="h-4 w-4 text-orca-400" />
            Node inspector
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-2 gap-2">
            <Button onClick={onAddNode}>
              <Plus className={iconClassName} />
              Agent
            </Button>
            <Button variant="secondary" onClick={onAddBranchNode}>
              <GitBranchPlus className={iconClassName} />
              Branch
            </Button>
          </div>
        </CardContent>
      </Card>
    )
  }
  const currentNode = node

  function setField(key: keyof WorkflowNode, value: string | string[]) {
    if (key === 'inputs' && Array.isArray(value)) {
      onUpdateNode({ ...currentNode, inputs: value, depends_on: value.filter((source) => source !== 'goal') })
      return
    }
    onUpdateNode({ ...currentNode, [key]: value })
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <PiListChecksDuotone className="h-4 w-4 text-orca-400" />
          Node inspector
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-3">
        <div className="grid grid-cols-2 gap-2">
          <Button onClick={onAddNode}>
            <Plus className={iconClassName} />
            Agent
          </Button>
          <Button variant="secondary" onClick={onAddBranchNode}>
            <GitBranchPlus className={iconClassName} />
            Branch
          </Button>
        </div>
        <label className={fieldLabelClassName}>
          <span className="text-slate-700">Node id</span>
          <Input value={currentNode.id} onChange={(event) => setField('id', event.target.value)} />
        </label>
        <div className="grid grid-cols-2 gap-2">
          <label className={fieldLabelClassName}>
            <span className="text-slate-700">Kind</span>
            <select
              className={selectClassName}
              value={currentNode.kind}
              onChange={(event) => setField('kind', event.target.value)}
            >
              <option value="agent">agent</option>
              <option value="branch">branch</option>
            </select>
          </label>
          <label className={fieldLabelClassName}>
            <span className="text-slate-700">Contract</span>
            <Input value={currentNode.output_contract} onChange={(event) => setField('output_contract', event.target.value)} />
          </label>
        </div>
        <label className={fieldLabelClassName}>
          <span className="text-slate-700">Instruction</span>
          <Input value={currentNode.instruction} onChange={(event) => setField('instruction', event.target.value)} />
        </label>
        {currentNode.kind === 'branch' ? (
          <label className={fieldLabelClassName}>
            <span className="text-slate-700">Evaluation</span>
            <Textarea
              className="min-h-20"
              value={currentNode.evaluation}
              onChange={(event) => setField('evaluation', event.target.value)}
            />
          </label>
        ) : null}
        <div className="grid grid-cols-2 gap-2">
          <label className={fieldLabelClassName}>
            <span className="text-slate-700">Backend</span>
            <select
              className={selectClassName}
              value={currentNode.backend}
              onChange={(event) => setField('backend', event.target.value)}
            >
              <option value="">Use default ({defaultBackend})</option>
              {withCurrentOption(backendOptions, currentNode.backend).map((backend) => (
                <option key={backend} value={backend}>
                  {backend}
                </option>
              ))}
            </select>
          </label>
          <label className={fieldLabelClassName}>
            <span className="text-slate-700">Model</span>
            <select
              className={selectClassName}
              value={currentNode.model}
              onChange={(event) => setField('model', event.target.value)}
            >
              <option value="">Use default ({defaultModel})</option>
              {withCurrentOption(modelOptions, currentNode.model).map((model) => (
                <option key={model} value={model}>
                  {model}
                </option>
              ))}
            </select>
          </label>
        </div>
        {modelOptions.length > 1 ? (
          <div className="flex flex-wrap gap-1 border-t border-sky-100 pt-2">
            {modelOptions.map((model) => {
              const active = currentNode.model === model || (!currentNode.model && model === defaultModel)
              return (
                <button
                  key={model}
                  type="button"
                  className={`rounded-md border px-2 py-1 text-[11px] font-medium ${
                    active
                      ? 'border-orca-500 bg-sky-50 text-sky-950'
                      : 'border-sky-100 bg-white text-slate-600 hover:bg-sky-50'
                  }`}
                  onClick={() => setField('model', model === defaultModel ? '' : model)}
                >
                  {model}
                </button>
              )
            })}
          </div>
        ) : null}
        <label className={fieldLabelClassName}>
          <span className="text-slate-700">Inputs</span>
          <select
            multiple
            className="min-h-24 w-full rounded-md border border-sky-200 bg-white px-2.5 py-2 text-xs text-slate-950 shadow-sm focus:border-orca-500 focus:outline-none focus:ring-1 focus:ring-orca-500"
            value={currentNode.inputs}
            onChange={(event) =>
              setField(
                'inputs',
                Array.from(event.currentTarget.selectedOptions).map((option) => option.value),
              )
            }
          >
            <option value="goal">goal</option>
            {nodes
              .filter((candidate) => candidate.id !== currentNode.id)
              .map((candidate) => (
                <option key={candidate.id} value={candidate.id}>
                  {candidate.id}
                </option>
              ))}
          </select>
        </label>
        <Button variant="outline" className="w-full" onClick={() => onRemoveNode(currentNode.id)}>
          <Trash2 className={iconClassName} />
          Remove node
        </Button>
      </CardContent>
    </Card>
  )
}

function ConnectionBuilder({
  nodes,
  onAddConnection,
}: {
  nodes: WorkflowNode[]
  onAddConnection: (from: string, to: string) => void
}) {
  const [from, setFrom] = useState('')
  const [to, setTo] = useState('')

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <GitBranchPlus className="h-4 w-4 text-orca-400" />
          Input link
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-3">
        <select
          className={selectClassName}
          value={from}
          onChange={(event) => setFrom(event.target.value)}
        >
          <option value="">From node</option>
          {nodes.map((node) => (
            <option key={node.id} value={node.id}>
              {node.id}
            </option>
          ))}
        </select>
        <select
          className={selectClassName}
          value={to}
          onChange={(event) => setTo(event.target.value)}
        >
          <option value="">To node</option>
          {nodes.map((node) => (
            <option key={node.id} value={node.id}>
              {node.id}
            </option>
          ))}
        </select>
        <Button className="w-full" variant="secondary" onClick={() => onAddConnection(from, to)}>
          <GitBranchPlus className={iconClassName} />
          Add input
        </Button>
      </CardContent>
    </Card>
  )
}

const GOAL_NODE_ID = '__goal__'

function layoutWorkflowCanvas(nodes: WorkflowNode[], positions: Record<string, Point>): LayoutNode[] {
  const goal = {
    id: GOAL_NODE_ID,
    label: 'Main goal',
    detail: 'workflow goal',
    kind: 'goal' as const,
    ...withNodeBounds(positions[GOAL_NODE_ID] ?? defaultGoalPosition(), 170, 88),
  }
  const agents = nodes.map((node, index) => ({
    id: node.id,
    label: node.id,
    detail: node.kind === 'branch' ? 'true / false' : node.output_contract || node.kind,
    kind: (node.kind === 'branch' ? 'branch' : 'agent') as CanvasNodeKind,
    agentId: node.id,
    dependsOn: node.depends_on,
    inputs: node.inputs,
    ...withNodeBounds(positions[node.id] ?? defaultAgentPosition(index), 190, 96),
  }))
  return [goal, ...agents]
}

function drawWorkflowCanvas(
  context: CanvasRenderingContext2D,
  width: number,
  height: number,
  nodes: LayoutNode[],
  connections: WorkflowConnection[],
  goalConnections: string[],
  selectedNodeIds: string[],
  selectedConnection: SelectedConnection | null,
  connecting: ConnectionDragState | null,
  hoverTarget: CanvasHitTarget | null,
  marquee: MarqueeState | null,
  viewport: Viewport,
) {
  context.clearRect(0, 0, width, height)
  context.fillStyle = '#f8fbff'
  context.fillRect(0, 0, width, height)
  context.strokeStyle = '#dbeafe'
  context.lineWidth = 1
  const gridSize = 32
  const firstGridX = Math.floor((-viewport.pan.x / viewport.zoom) / gridSize) * gridSize
  const lastGridX = Math.ceil(((width - viewport.pan.x) / viewport.zoom) / gridSize) * gridSize
  const firstGridY = Math.floor((-viewport.pan.y / viewport.zoom) / gridSize) * gridSize
  const lastGridY = Math.ceil(((height - viewport.pan.y) / viewport.zoom) / gridSize) * gridSize
  for (let worldX = firstGridX; worldX <= lastGridX; worldX += gridSize) {
    const x = viewport.pan.x + worldX * viewport.zoom
    context.beginPath()
    context.moveTo(x, 0)
    context.lineTo(x, height)
    context.stroke()
  }
  for (let worldY = firstGridY; worldY <= lastGridY; worldY += gridSize) {
    const y = viewport.pan.y + worldY * viewport.zoom
    context.beginPath()
    context.moveTo(0, y)
    context.lineTo(width, y)
    context.stroke()
  }

  context.save()
  context.translate(viewport.pan.x, viewport.pan.y)
  context.scale(viewport.zoom, viewport.zoom)

  const byId = new Map(nodes.map((node) => [node.id, node]))
  const visibleEdges = buildVisibleCanvasEdges(nodes, connections, goalConnections)
  const hoveredConnection = hoverTarget?.kind === 'edge' ? hoverTarget.connection : null
  const hoveredNodeId = hoverTarget?.kind === 'node' || hoverTarget?.kind === 'input' || hoverTarget?.kind === 'output' ? hoverTarget.nodeId : null
  const emphasizedNodeIds = new Set(selectedNodeIds)
  if (hoveredNodeId) emphasizedNodeIds.add(hoveredNodeId)
  if (selectedConnection) {
    emphasizedNodeIds.add(selectedConnection.fromId)
    emphasizedNodeIds.add(selectedConnection.toId)
  }
  if (hoveredConnection) {
    emphasizedNodeIds.add(hoveredConnection.fromId)
    emphasizedNodeIds.add(hoveredConnection.toId)
  }
  const fadeUnrelatedEdges = visibleEdges.length > 8 && emphasizedNodeIds.size > 0
  const orderedEdges = [...visibleEdges].sort((left, right) => Number(isActiveCanvasEdge(left, selectedConnection, hoveredConnection)) - Number(isActiveCanvasEdge(right, selectedConnection, hoveredConnection)))
  for (const edge of orderedEdges) {
    const from = byId.get(edge.fromId)
    const to = byId.get(edge.toId)
    if (!from || !to) continue
    const selected = edge.selectable && connectionsMatch(edge, selectedConnection)
    const hovered = edge.selectable && connectionsMatch(edge, hoveredConnection)
    const related = !fadeUnrelatedEdges || emphasizedNodeIds.has(edge.fromId) || emphasizedNodeIds.has(edge.toId)
    context.globalAlpha = related ? 1 : 0.18
    context.strokeStyle = selected || hovered ? '#f97316' : edge.fromId === GOAL_NODE_ID ? '#eab308' : '#0ea5e9'
    context.lineWidth = selected || hovered ? 4 : related ? 2 : 1.25
    const inputPoint = inputHandleCenter(to, edge.inputIndex)
    drawEdge(context, outputHandleCenter(from, edge.condition), inputPoint, edge.offset)
    if (edge.condition && related) drawEdgeLabel(context, outputHandleCenter(from, edge.condition), inputPoint, edge.condition, edge.offset)
  }
  context.globalAlpha = 1

  if (connecting) {
    const from = byId.get(connecting.fromId)
    if (from) {
      context.strokeStyle = '#f97316'
      context.lineWidth = 2
      drawEdge(context, outputHandleCenter(from, connecting.condition), connecting.pointer)
    }
  }

  const connectionTargetId = connecting
    ? findConnectionTarget(nodes, connecting.fromId, connecting.pointer.x, connecting.pointer.y)?.node.id
    : null

  for (const node of nodes) {
    const selected = node.kind !== 'goal' && selectedNodeIds.includes(node.id)
    const hovered = hoverTarget?.kind === 'node' && hoverTarget.nodeId === node.id
    const connectionTarget = node.kind !== 'goal' && node.id === connectionTargetId
    context.fillStyle = nodeFill(node.kind, selected || connectionTarget || hovered)
    context.strokeStyle = connectionTarget ? '#f97316' : selected || hovered ? '#0284c7' : nodeStroke(node.kind)
    context.lineWidth = selected || connectionTarget || hovered ? 3 : 1
    roundRect(context, node.x, node.y, node.width, node.height, 8)
    context.fill()
    context.stroke()
    context.fillStyle = '#0f172a'
    context.font = '600 13px sans-serif'
    context.fillText(node.label, node.x + 12, node.y + 24, node.width - 24)
    context.fillStyle = node.kind === 'goal' ? '#854d0e' : node.kind === 'branch' ? '#a16207' : '#0369a1'
    context.font = '12px sans-serif'
    context.fillText(node.detail, node.x + 12, node.y + 48, node.width - 24)
    if (node.kind !== 'goal') drawInputHandles(context, node, hoverTarget)
    drawOutputHandles(context, node, hoverTarget)
  }
  if (marquee) drawSelectionRect(context, marquee)
  context.restore()
}

function screenToWorld(point: Point, viewport: Viewport): Point {
  return { x: (point.x - viewport.pan.x) / viewport.zoom, y: (point.y - viewport.pan.y) / viewport.zoom }
}

function zoomViewport(viewport: Viewport, anchor: Point, deltaY: number): Viewport {
  const worldBefore = screenToWorld(anchor, viewport)
  const zoomFactor = Math.exp(-deltaY * 0.001)
  const nextZoom = clamp(viewport.zoom * zoomFactor, MIN_CANVAS_ZOOM, MAX_CANVAS_ZOOM)
  return {
    zoom: nextZoom,
    pan: {
      x: anchor.x - worldBefore.x * nextZoom,
      y: anchor.y - worldBefore.y * nextZoom,
    },
  }
}

function clamp(value: number, minimum: number, maximum: number) {
  return Math.min(maximum, Math.max(minimum, value))
}

function hitTestCanvasEdge(
  nodes: LayoutNode[],
  connections: WorkflowConnection[],
  goalConnections: string[],
  x: number,
  y: number,
): SelectedConnection | null {
  const byId = new Map(nodes.map((node) => [node.id, node]))
  for (const edge of buildVisibleCanvasEdges(nodes, connections, goalConnections)) {
    const from = byId.get(edge.fromId)
    const to = byId.get(edge.toId)
    if (!from || !to) continue
    if (distanceToCubic(outputHandleCenter(from, edge.condition), inputHandleCenter(to, edge.inputIndex), { x, y }, edge.offset) <= 10) {
      return { fromId: edge.fromId, toId: edge.toId, condition: edge.condition, inputIndex: edge.inputIndex }
    }
  }
  return null
}

function canvasHitTarget(
  nodes: LayoutNode[],
  connections: WorkflowConnection[],
  goalConnections: string[],
  x: number,
  y: number,
): CanvasHitTarget {
  const output = findOutputHandle(nodes, x, y)
  if (output) return { kind: 'output', nodeId: output.node.id, condition: output.condition }
  const input = findInputHandle(nodes, x, y)
  if (input) return { kind: 'input', nodeId: input.node.id, index: input.index, isAppend: input.isAppend }
  const node = findNodeAt(nodes, x, y)
  if (node) return { kind: 'node', nodeId: node.id }
  const edge = hitTestCanvasEdge(nodes, connections, goalConnections, x, y)
  if (edge) return { kind: 'edge', connection: edge }
  return { kind: 'canvas' }
}

function buildVisibleCanvasEdges(
  nodes: LayoutNode[],
  connections: WorkflowConnection[],
  goalConnections: string[],
): CanvasEdge[] {
  const inputEdges = collectInputEdges(nodes)
  const dependencyEdges = collectDependencyEdges(nodes, inputEdges)
  const edges = [
    ...inputEdges,
    ...dependencyEdges.map(([fromId, toId]) => ({ fromId, toId })),
    ...connections.flatMap((connection) => connection.to.map((target) => ({ fromId: connection.from, toId: target, condition: connection.condition }))),
  ]
  const visibleEdges: CanvasEdge[] = [
    ...edges.map((edge) => ({ ...edge, selectable: true })),
    ...goalConnections
      .filter((toId) => !inputEdges.some((edge) => edge.fromId === GOAL_NODE_ID && edge.toId === toId))
      .map((toId) => ({ fromId: GOAL_NODE_ID, toId, selectable: true })),
  ]
  return withParallelEdgeOffsets(visibleEdges)
}

function withParallelEdgeOffsets(edges: CanvasEdge[]) {
  const groups = new Map<string, CanvasEdge[]>()
  for (const edge of edges) {
    const key = `${edge.fromId}->${edge.toId}`
    groups.set(key, [...(groups.get(key) ?? []), edge])
  }
  return edges.map((edge) => {
    const group = groups.get(`${edge.fromId}->${edge.toId}`) ?? []
    if (group.length <= 1) return { ...edge, offset: 0 }
    const index = group.indexOf(edge)
    return { ...edge, offset: (index - (group.length - 1) / 2) * 14 }
  })
}

function isActiveCanvasEdge(edge: CanvasEdge, selected: SelectedConnection | null, hovered: SelectedConnection | null) {
  return connectionsMatch(edge, selected) || connectionsMatch(edge, hovered)
}

function connectionsMatch(left: SelectedConnection | null | undefined, right: SelectedConnection | null | undefined) {
  return Boolean(
    left &&
      right &&
      left.fromId === right.fromId &&
      left.toId === right.toId &&
      left.condition === right.condition &&
      left.inputIndex === right.inputIndex,
  )
}

function collectInputEdges(nodes: LayoutNode[]) {
  const nodeIds = new Set(nodes.filter((node) => node.kind !== 'goal').map((node) => node.id))
  return nodes.flatMap((node) => {
    if (node.kind === 'goal' || !node.agentId) return []
    return (node.inputs ?? [])
      .map((source, inputIndex) => ({ fromId: source === 'goal' ? GOAL_NODE_ID : source, toId: node.id, inputIndex }))
      .filter((edge) => edge.fromId === GOAL_NODE_ID || nodeIds.has(edge.fromId))
  })
}

function collectDependencyEdges(nodes: LayoutNode[], inputEdges: Array<{ fromId: string; toId: string }>) {
  const nodeIds = new Set(nodes.filter((node) => node.kind !== 'goal').map((node) => node.id))
  const inputEdgeIds = new Set(inputEdges.map((edge) => `${edge.fromId}->${edge.toId}`))
  return nodes.flatMap((node) => {
    if (node.kind === 'goal' || !node.agentId) return []
    return (node.dependsOn ?? [])
      .filter((dependency) => nodeIds.has(dependency))
      .filter((dependency) => !inputEdgeIds.has(`${dependency}->${node.id}`))
      .map((dependency) => [dependency, node.id] as const)
  })
}

function withNodeBounds(position: Point, width: number, height: number) {
  return { x: position.x, y: position.y, width, height }
}

function defaultGoalPosition(): Point {
  return { x: 42, y: 72 }
}

function defaultAgentPosition(index: number): Point {
  return { x: 260, y: 72 + index * 132 }
}

function inputHandleCenter(node: LayoutNode, index?: number): Point {
  const inputs = node.inputs ?? []
  if (node.kind !== 'goal' && typeof index === 'number') {
    const slots = Math.max(1, inputs.length + 1)
    return { x: node.x, y: node.y + ((index + 1) * node.height) / (slots + 1) }
  }
  return { x: node.x, y: node.y + node.height / 2 }
}

function inputHandles(node: LayoutNode): InputHandleHit[] {
  if (node.kind === 'goal') return []
  const inputCount = node.inputs?.length ?? 0
  return Array.from({ length: inputCount + 1 }, (_, index) => ({ node, index, isAppend: index === inputCount }))
}

function outputHandleCenter(node: LayoutNode, condition?: 'true' | 'false'): Point {
  if (node.kind === 'branch' && condition === 'true') return { x: node.x + node.width, y: node.y + node.height * 0.34 }
  if (node.kind === 'branch' && condition === 'false') return { x: node.x + node.width, y: node.y + node.height * 0.66 }
  return { x: node.x + node.width, y: node.y + node.height / 2 }
}

function outputHandles(node: LayoutNode): OutputHandleHit[] {
  if (node.kind === 'branch') return [{ node, condition: 'true' }, { node, condition: 'false' }]
  return [{ node }]
}

function findOutputHandle(nodes: LayoutNode[], x: number, y: number): OutputHandleHit | null {
  for (const node of [...nodes].reverse()) {
    for (const handle of outputHandles(node)) {
      if (distance(outputHandleCenter(node, handle.condition), { x, y }) <= 12) return handle
    }
  }
  return null
}

function findInputHandle(nodes: LayoutNode[], x: number, y: number): InputHandleHit | null {
  for (const node of [...nodes].reverse()) {
    for (const handle of inputHandles(node)) {
      if (distance(inputHandleCenter(node, handle.index), { x, y }) <= 14) return handle
    }
  }
  return null
}

function findNodeAt(nodes: LayoutNode[], x: number, y: number) {
  return [...nodes].reverse().find((node) =>
    x >= node.x && x <= node.x + node.width && y >= node.y && y <= node.y + node.height,
  )
}

function isInOutputHandle(node: LayoutNode, x: number, y: number) {
  return outputHandles(node).some((handle) => distance(outputHandleCenter(node, handle.condition), { x, y }) <= 12)
}

function findConnectionTarget(nodes: LayoutNode[], fromId: string, x: number, y: number): InputHandleHit | null {
  for (const node of [...nodes].reverse()) {
    if (node.kind === 'goal' || node.id === fromId) continue
    for (const handle of inputHandles(node)) {
      if (distance(inputHandleCenter(node, handle.index), { x, y }) <= 14) return handle
    }
    if (x >= node.x && x <= node.x + node.width && y >= node.y && y <= node.y + node.height && !isInOutputHandle(node, x, y)) {
      return { node, index: node.inputs?.length ?? 0, isAppend: true }
    }
  }
  return null
}

function distance(a: Point, b: Point) {
  return Math.hypot(a.x - b.x, a.y - b.y)
}

function distanceToCubic(from: Point, to: Point, point: Point, offset = 0) {
  let minimum = Number.POSITIVE_INFINITY
  let previous = from
  const controls = edgeControls(from, to, offset)
  for (let step = 1; step <= 24; step += 1) {
    const current = cubicPoint(from, controls.controlA, controls.controlB, to, step / 24)
    minimum = Math.min(minimum, distanceToSegment(point, previous, current))
    previous = current
  }
  return minimum
}

function edgeControls(from: Point, to: Point, offset = 0) {
  const controlDistance = Math.max(48, Math.min(180, Math.abs(to.x - from.x) * 0.5))
  return {
    controlA: { x: from.x + controlDistance, y: from.y + offset },
    controlB: { x: to.x - controlDistance, y: to.y + offset },
  }
}

function cubicPoint(start: Point, controlA: Point, controlB: Point, end: Point, time: number) {
  const inverse = 1 - time
  return {
    x:
      inverse ** 3 * start.x +
      3 * inverse ** 2 * time * controlA.x +
      3 * inverse * time ** 2 * controlB.x +
      time ** 3 * end.x,
    y:
      inverse ** 3 * start.y +
      3 * inverse ** 2 * time * controlA.y +
      3 * inverse * time ** 2 * controlB.y +
      time ** 3 * end.y,
  }
}

function distanceToSegment(point: Point, start: Point, end: Point) {
  const lengthSquared = (end.x - start.x) ** 2 + (end.y - start.y) ** 2
  if (lengthSquared === 0) return distance(point, start)
  const time = Math.max(0, Math.min(1, ((point.x - start.x) * (end.x - start.x) + (point.y - start.y) * (end.y - start.y)) / lengthSquared))
  return distance(point, { x: start.x + time * (end.x - start.x), y: start.y + time * (end.y - start.y) })
}

function drawEdge(context: CanvasRenderingContext2D, from: Point, to: Point, offset = 0) {
  const controls = edgeControls(from, to, offset)
  context.beginPath()
  context.moveTo(from.x, from.y)
  context.bezierCurveTo(controls.controlA.x, controls.controlA.y, controls.controlB.x, controls.controlB.y, to.x, to.y)
  context.stroke()
  context.beginPath()
  context.moveTo(to.x, to.y)
  context.lineTo(to.x - 8, to.y - 5)
  context.lineTo(to.x - 8, to.y + 5)
  context.closePath()
  context.fillStyle = context.strokeStyle.toString()
  context.fill()
}

function drawEdgeLabel(context: CanvasRenderingContext2D, from: Point, to: Point, label: string, offset = 0) {
  const controls = edgeControls(from, to, offset)
  const point = cubicPoint(from, controls.controlA, controls.controlB, to, 0.5)
  const text = label === 'true' ? 'True' : 'False'
  context.save()
  context.font = '600 11px sans-serif'
  const width = context.measureText(text).width + 14
  context.fillStyle = '#fefce8'
  context.strokeStyle = '#facc15'
  context.lineWidth = 1
  roundRect(context, point.x - width / 2, point.y - 12, width, 20, 6)
  context.fill()
  context.stroke()
  context.fillStyle = '#854d0e'
  context.fillText(text, point.x - width / 2 + 7, point.y + 2)
  context.restore()
}

function drawHandle(context: CanvasRenderingContext2D, center: Point, fill: string, stroke: string, radius = 7) {
  context.beginPath()
  context.arc(center.x, center.y, radius, 0, Math.PI * 2)
  context.fillStyle = fill
  context.strokeStyle = stroke
  context.lineWidth = 2
  context.fill()
  context.stroke()
}

function drawInputHandles(context: CanvasRenderingContext2D, node: LayoutNode, hoverTarget: CanvasHitTarget | null) {
  for (const handle of inputHandles(node)) {
    const center = inputHandleCenter(node, handle.index)
    const hovered = hoverTarget?.kind === 'input' && hoverTarget.nodeId === node.id && hoverTarget.index === handle.index
    drawHandle(context, center, handle.isAppend ? '#f8fafc' : '#ffffff', hovered ? '#f97316' : nodeStroke(node.kind), hovered ? 8.5 : 7)
    context.fillStyle = handle.isAppend ? '#64748b' : '#0f172a'
    context.font = '700 9px sans-serif'
    context.textAlign = 'center'
    context.textBaseline = 'middle'
    context.fillText(handle.isAppend ? '+' : String(handle.index + 1), center.x, center.y + 0.5)
    context.textAlign = 'start'
    context.textBaseline = 'alphabetic'
  }
}

function drawOutputHandles(context: CanvasRenderingContext2D, node: LayoutNode, hoverTarget: CanvasHitTarget | null) {
  for (const handle of outputHandles(node)) {
    const center = outputHandleCenter(node, handle.condition)
    const hovered = hoverTarget?.kind === 'output' && hoverTarget.nodeId === node.id && hoverTarget.condition === handle.condition
    drawHandle(context, center, '#0ea5e9', hovered ? '#f97316' : '#0284c7', hovered ? 8.5 : 7)
    if (handle.condition) {
      context.fillStyle = '#ffffff'
      context.font = '700 9px sans-serif'
      context.textAlign = 'center'
      context.textBaseline = 'middle'
      context.fillText(handle.condition === 'true' ? 'T' : 'F', center.x, center.y + 0.5)
      context.textAlign = 'start'
      context.textBaseline = 'alphabetic'
    }
  }
}

function normalizedRect(start: Point, current: Point) {
  return {
    x: Math.min(start.x, current.x),
    y: Math.min(start.y, current.y),
    width: Math.abs(start.x - current.x),
    height: Math.abs(start.y - current.y),
  }
}

function rectIntersectsNode(rect: { x: number; y: number; width: number; height: number }, node: LayoutNode) {
  return rect.x <= node.x + node.width && rect.x + rect.width >= node.x && rect.y <= node.y + node.height && rect.y + rect.height >= node.y
}

function drawSelectionRect(context: CanvasRenderingContext2D, marquee: MarqueeState) {
  const rect = normalizedRect(marquee.start, marquee.current)
  context.save()
  context.fillStyle = 'rgb(14 165 233 / 0.10)'
  context.strokeStyle = '#0284c7'
  context.lineWidth = 1
  context.setLineDash([5, 4])
  context.fillRect(rect.x, rect.y, rect.width, rect.height)
  context.strokeRect(rect.x, rect.y, rect.width, rect.height)
  context.restore()
}

function canvasCursorClassName({
  connecting,
  dragging,
  panning,
  marquee,
  hoverTarget,
  connectionTarget,
  spacePressed,
}: {
  connecting: ConnectionDragState | null
  dragging: DragState | null
  panning: boolean
  marquee: MarqueeState | null
  hoverTarget: CanvasHitTarget | null
  connectionTarget: InputHandleHit | null
  spacePressed: boolean
}) {
  if (connecting) return connectionTarget ? 'cursor-crosshair' : 'cursor-not-allowed'
  if (dragging) return 'cursor-move'
  if (panning) return 'cursor-grabbing'
  if (marquee) return 'cursor-crosshair'
  if (spacePressed) return 'cursor-grab'
  if (hoverTarget?.kind === 'node') return 'cursor-move'
  if (hoverTarget?.kind === 'output' || hoverTarget?.kind === 'input' || hoverTarget?.kind === 'edge') return 'cursor-pointer'
  return 'cursor-default'
}

function nodeFill(kind: CanvasNodeKind, selected: boolean) {
  if (selected) return '#e0f2fe'
  if (kind === 'goal') return '#fefce8'
  if (kind === 'branch') return '#fffbeb'
  return '#ffffff'
}

function nodeStroke(kind: CanvasNodeKind) {
  if (kind === 'goal') return '#eab308'
  if (kind === 'branch') return '#facc15'
  return '#bae6fd'
}

function renamePosition(positions: Record<string, Point>, oldId: string, newId: string) {
  const next = { ...positions }
  if (next[oldId] && !next[newId]) {
    next[newId] = next[oldId]
    delete next[oldId]
  }
  return next
}

function withOrderedInput(node: WorkflowNode, source: string, inputIndex?: number): WorkflowNode {
  const nextInputs = node.inputs.filter((existing) => existing !== source)
  const index = typeof inputIndex === 'number' ? Math.max(0, Math.min(inputIndex, nextInputs.length)) : nextInputs.length
  nextInputs.splice(index, 0, source)
  const nextDependsOn = source === 'goal' || node.depends_on.includes(source) ? node.depends_on : [...node.depends_on, source]
  return { ...node, inputs: nextInputs, depends_on: nextDependsOn }
}

function withoutOrderedInput(node: WorkflowNode, source: string): WorkflowNode {
  return {
    ...node,
    inputs: node.inputs.filter((existing) => existing !== source),
    depends_on: source === 'goal' ? node.depends_on : node.depends_on.filter((dependency) => dependency !== source),
  }
}

function omitPosition(positions: Record<string, Point>, id: string) {
  const { [id]: _removed, ...rest } = positions
  return rest
}

function roundRect(context: CanvasRenderingContext2D, x: number, y: number, width: number, height: number, radius: number) {
  context.beginPath()
  context.moveTo(x + radius, y)
  context.lineTo(x + width - radius, y)
  context.quadraticCurveTo(x + width, y, x + width, y + radius)
  context.lineTo(x + width, y + height - radius)
  context.quadraticCurveTo(x + width, y + height, x + width - radius, y + height)
  context.lineTo(x + radius, y + height)
  context.quadraticCurveTo(x, y + height, x, y + height - radius)
  context.lineTo(x, y + radius)
  context.quadraticCurveTo(x, y, x + radius, y)
  context.closePath()
}

function parseWorkflow(content: string): ParsedWorkflow {
  const nodes = Array.from(content.matchAll(/\[\[orchestration\.nodes\]\]([\s\S]*?)(?=\n\[\[|\n\[|$)/g)).map(
    (match) => parseWorkflowNode(match[1]),
  )
  const defaultsBlock = content.match(/\[orchestration\.defaults\]([\s\S]*?)(?=\n\[\[|\n\[|$)/)?.[1] ?? ''
  const defaults = {
    backend: stringField(defaultsBlock, 'backend'),
    model: stringField(defaultsBlock, 'model'),
  }
  const backends = Array.from(content.matchAll(/\[\[orchestration\.backends\]\]([\s\S]*?)(?=\n\[\[|\n\[|$)/g))
    .map((match) => ({
      id: stringField(match[1], 'id'),
      program: stringField(match[1], 'program'),
      args: arrayField(match[1], 'args'),
    }))
    .filter((backend) => backend.id)
  const connections = Array.from(content.matchAll(/\[\[orchestration\.connections\]\]([\s\S]*?)(?=\n\[\[|\n\[|$)/g))
    .map((match) => ({
      from: stringField(match[1], 'from'),
      to: arrayField(match[1], 'to'),
      condition: branchConditionField(match[1], 'condition'),
    }))
    .filter((connection) => connection.from && connection.to.length > 0)
  return { nodes, connections, backends, defaults }
}

function parseWorkflowNode(block: string): WorkflowNode {
  return {
    id: stringField(block, 'id') || 'agent',
    kind: stringField(block, 'kind') || 'agent',
    evaluation: stringField(block, 'evaluation'),
    output_contract: stringField(block, 'output_contract') || 'implementation',
    instruction: stringField(block, 'instruction'),
    backend: stringField(block, 'backend'),
    model: stringField(block, 'model'),
    depends_on: arrayField(block, 'depends_on'),
    inputs: inputSourcesField(block),
  }
}

function inputSourcesField(block: string) {
  const raw = block.match(/^inputs\s*=\s*\[([\s\S]*?)\]\s*$/m)?.[1] ?? ''
  return Array.from(raw.matchAll(/source\s*=\s*"([^"]+)"/g)).map((match) => match[1])
}

function branchConditionField(block: string, key: string): 'true' | 'false' | undefined {
  const value = stringField(block, key)
  return value === 'true' || value === 'false' ? value : undefined
}

function stringField(block: string, key: string) {
  return block.match(new RegExp(`^${key}\\s*=\\s*"([^"]*)"`, 'm'))?.[1] ?? ''
}

function arrayField(block: string, key: string) {
  const raw = block.match(new RegExp(`^${key}\\s*=\\s*\\[([^\\]]*)\\]`, 'm'))?.[1] ?? ''
  return Array.from(raw.matchAll(/"([^"]+)"/g)).map((match) => match[1])
}

function serializeWorkflow(
  nodes: WorkflowNode[],
  connections: WorkflowConnection[],
  defaults: WorkflowDefaults = { backend: 'copilot', model: 'default' },
  backends: WorkflowBackend[] = [],
) {
  const lines = ['[orchestration]', 'max_parallel_agents = 8', 'approval_mode = "auto"', '']
  if (defaults.backend || defaults.model) {
    lines.push('[orchestration.defaults]')
    if (defaults.backend) lines.push(`backend = "${escapeToml(defaults.backend)}"`)
    if (defaults.model) lines.push(`model = "${escapeToml(defaults.model)}"`)
    lines.push('')
  }
  for (const backend of backends) {
    lines.push('[[orchestration.backends]]')
    lines.push(`id = "${escapeToml(backend.id)}"`)
    if (backend.program) lines.push(`program = "${escapeToml(backend.program)}"`)
    if (backend.args.length > 0) lines.push(`args = [${backend.args.map(quotedToml).join(', ')}]`)
    lines.push('')
  }
  for (const node of nodes) {
    lines.push('[[orchestration.nodes]]')
    lines.push(`id = "${escapeToml(node.id)}"`)
    if (node.kind && node.kind !== 'agent') lines.push(`kind = "${escapeToml(node.kind)}"`)
    if (node.kind === 'branch' && node.evaluation) lines.push(`evaluation = "${escapeToml(node.evaluation)}"`)
    lines.push(`output_contract = "${escapeToml(node.output_contract || 'implementation')}"`)
    if (node.instruction) lines.push(`instruction = "${escapeToml(node.instruction)}"`)
    if (node.backend) lines.push(`backend = "${escapeToml(node.backend)}"`)
    if (node.model) lines.push(`model = "${escapeToml(node.model)}"`)
    if (node.inputs.length > 0) lines.push(`inputs = [${node.inputs.map((source) => `{ source = ${quotedToml(source)} }`).join(', ')}]`)
    if (node.depends_on.length > 0) lines.push(`depends_on = [${node.depends_on.map(quotedToml).join(', ')}]`)
    lines.push('')
  }
  for (const connection of connections) {
    lines.push('[[orchestration.connections]]')
    lines.push(`from = "${escapeToml(connection.from)}"`)
    if (connection.condition) lines.push(`condition = "${connection.condition}"`)
    lines.push(`to = [${connection.to.map(quotedToml).join(', ')}]`)
    lines.push('')
  }
  return lines.join('\n')
}

function workflowBackendOptions(workflow: ParsedWorkflow) {
  return uniqueStrings([
    'copilot',
    workflow.defaults.backend,
    ...workflow.backends.map((backend) => backend.id),
    ...workflow.nodes.map((node) => node.backend),
  ])
}

function workflowModelOptions(workflow: ParsedWorkflow) {
  return uniqueStrings([
    'default',
    workflow.defaults.model,
    ...workflow.nodes.map((node) => node.model),
  ])
}

function uniqueStrings(values: string[]) {
  return Array.from(new Set(values.map((value) => value.trim()).filter(Boolean))).sort((left, right) => {
    if (left === 'default' || left === 'copilot') return -1
    if (right === 'default' || right === 'copilot') return 1
    return left.localeCompare(right)
  })
}

function withCurrentOption(options: string[], current: string) {
  return uniqueStrings([...options, current])
}

function nextBranchCondition(connections: WorkflowConnection[], from: string): 'true' | 'false' {
  const used = new Set(connections.filter((connection) => connection.from === from).map((connection) => connection.condition))
  return used.has('true') && !used.has('false') ? 'false' : 'true'
}

function upsertWorkflowConnection(
  connections: WorkflowConnection[],
  from: string,
  to: string,
  condition?: 'true' | 'false',
) {
  const existing = connections.find((connection) => connection.from === from && connection.condition === condition)
  if (!existing) return [...connections, { from, to: [to], condition }]
  return connections.map((connection) =>
    connection === existing && !connection.to.includes(to) ? { ...connection, to: [...connection.to, to] } : connection,
  )
}

function uniqueNodeId(nodes: WorkflowNode[], prefix: string) {
  const ids = new Set(nodes.map((node) => node.id))
  let index = nodes.length + 1
  let id = `${prefix}-${index}`
  while (ids.has(id)) {
    index += 1
    id = `${prefix}-${index}`
  }
  return id
}

function quotedToml(value: string) {
  return `"${escapeToml(value)}"`
}

function escapeToml(value: string) {
  return value.replace(/\\/g, '\\\\').replace(/"/g, '\\"')
}

type ArtifactsPageProps = {
  artifactRoots: string[]
  selectedArtifact: string
  artifactFiles: string[]
  selectedArtifactFile: string
  artifactContent: string
  setSelectedArtifact: (path: string) => void
  setSelectedArtifactFile: (path: string) => void
  onLoadArtifacts: () => void
  onListArtifactFiles: () => void
  onReadArtifact: () => void
}

function ArtifactsPage({
  artifactRoots,
  selectedArtifact,
  artifactFiles,
  selectedArtifactFile,
  artifactContent,
  setSelectedArtifact,
  setSelectedArtifactFile,
  onLoadArtifacts,
  onListArtifactFiles,
  onReadArtifact,
}: ArtifactsPageProps) {
  return (
    <main className="grid min-h-0 flex-1 grid-cols-[320px_1fr]">
      <aside className={`${panelClassName} border-r border-sky-100`}>
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <FolderArchive className="h-4 w-4 text-orca-400" />
              Artifact runs
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex gap-2">
              <Button className="flex-1" variant="secondary" onClick={onLoadArtifacts}>
                <FolderArchive className={iconClassName} />
                Runs
              </Button>
              <Button className="flex-1" variant="secondary" onClick={onListArtifactFiles}>
                <PiFilesDuotone className={iconClassName} />
                Files
              </Button>
              <Button className="flex-1" variant="outline" onClick={onReadArtifact}>
                <PiFileTextDuotone className={iconClassName} />
                Read
              </Button>
            </div>
            <select
              className={selectClassName}
              value={selectedArtifact}
              onChange={(event) => setSelectedArtifact(event.target.value)}
            >
              <option value="">Select artifact run</option>
              {artifactRoots.map((root) => (
                <option key={root} value={root}>
                  {root}
                </option>
              ))}
            </select>
            <select
              className={selectClassName}
              value={selectedArtifactFile}
              onChange={(event) => setSelectedArtifactFile(event.target.value)}
            >
              <option value="">Select artifact file</option>
              {artifactFiles.map((file) => (
                <option key={file} value={file}>
                  {file}
                </option>
              ))}
            </select>
          </CardContent>
        </Card>
      </aside>
      <section className="min-h-0 overflow-hidden p-3">
        <Card className="h-full min-h-0">
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <PiFolderOpenDuotone className="h-4 w-4 text-orca-400" />
              Artifact explorer
            </CardTitle>
          </CardHeader>
          <CardContent className="h-[calc(100%-5rem)] min-h-0">
            <pre className="h-full overflow-auto whitespace-pre-wrap rounded-md border border-sky-200 bg-white p-3 text-[11px] leading-5 text-slate-700 shadow-sm">
              {artifactContent || 'Artifact manifest content will appear here.'}
            </pre>
          </CardContent>
        </Card>
      </section>
    </main>
  )
}

type SettingsPanelProps = {
  settings: Settings
  settingsPath: string
  setSettings: (settings: Settings) => void
  setSettingsPath: (path: string) => void
  onSave: () => void
  onReload: () => void
  onClose?: () => void
}

function SettingsPanel({
  settings,
  settingsPath,
  setSettings,
  setSettingsPath,
  onSave,
  onReload,
  onClose,
}: SettingsPanelProps) {
  function setSourceList(key: keyof Settings['sources'], values: string[]) {
    setSettings({
      ...settings,
      sources: {
        ...settings.sources,
        [key]: values.map((value) => value.trim()).filter(Boolean),
      },
    })
  }

  function appendSource(key: keyof Settings['sources'], path: string) {
    const value = path.trim()
    if (!value) return
    const values = settings.sources[key] ?? []
    if (values.includes(value)) return
    setSourceList(key, [...values, value])
  }

  async function browseSource(key: keyof Settings['sources']) {
    const path = await browseDirectory()
    if (path) {
      appendSource(key, path)
    }
  }

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex items-center justify-between gap-3 border-b border-sky-100 px-3 py-2.5">
        <h2 className="flex items-center gap-2 text-sm font-semibold text-slate-950">
          <SettingsIcon className="h-4 w-4 text-orca-400" />
          Settings
        </h2>
        {onClose ? (
          <Button variant="ghost" size="sm" className="px-2" aria-label="Close settings" onClick={onClose}>
            <X className="h-4 w-4" />
          </Button>
        ) : null}
      </div>
      <div className="min-h-0 flex-1 space-y-2 overflow-auto px-3 py-2.5">
        <label className={fieldLabelClassName}>
          <span>Settings file</span>
          <Input className={settingsInputClassName} value={settingsPath} onChange={(event) => setSettingsPath(event.target.value)} />
        </label>
        <SourceListEditor
          title="Agent sources"
          values={settings.sources.agents ?? []}
          placeholder="agents/custom"
          onChange={(values) => setSourceList('agents', values)}
          onAdd={(path) => appendSource('agents', path)}
          onBrowse={() => browseSource('agents')}
        />
        <SourceListEditor
          title="Instruction sources"
          values={settings.sources.instructions ?? []}
          placeholder="config/instructions"
          onChange={(values) => setSourceList('instructions', values)}
          onAdd={(path) => appendSource('instructions', path)}
          onBrowse={() => browseSource('instructions')}
        />
        <SourceListEditor
          title="Skill sources"
          values={settings.sources.skills ?? []}
          placeholder="skills/custom"
          onChange={(values) => setSourceList('skills', values)}
          onAdd={(path) => appendSource('skills', path)}
          onBrowse={() => browseSource('skills')}
        />
        <SourceListEditor
          title="Workflow sources"
          values={settings.sources.workflows ?? []}
          placeholder="config/workflows"
          onChange={(values) => setSourceList('workflows', values)}
          onAdd={(path) => appendSource('workflows', path)}
          onBrowse={() => browseSource('workflows')}
        />
        <label className="grid gap-1 border-t border-sky-100 pt-2 text-xs font-medium text-slate-700">
          <span>Default workflow</span>
          <Input
            className={settingsInputClassName}
            value={settings.defaults.workflow ?? ''}
            onChange={(event) =>
              setSettings({
                ...settings,
                defaults: { ...settings.defaults, workflow: event.target.value || null },
              })
            }
          />
        </label>
        <label className={fieldLabelClassName}>
          <span>Default artifact dir</span>
          <Input
            className={settingsInputClassName}
            value={settings.defaults.artifact_dir ?? ''}
            onChange={(event) =>
              setSettings({
                ...settings,
                defaults: { ...settings.defaults, artifact_dir: event.target.value || null },
              })
            }
          />
        </label>
        <label className={fieldLabelClassName}>
          <span>Default max parallel agents</span>
          <Input
            className={settingsInputClassName}
            value={settings.defaults.max_parallel_agents?.toString() ?? ''}
            onChange={(event) =>
              setSettings({
                ...settings,
                defaults: {
                  ...settings.defaults,
                  max_parallel_agents: event.target.value ? Number(event.target.value) : null,
                },
              })
            }
          />
        </label>
        <div className="flex gap-2 border-t border-sky-100 pt-2">
          <Button size="sm" className="flex-1" onClick={onSave}>
            <Save className={iconClassName} />
            Save
          </Button>
          <Button size="sm" className="flex-1" variant="secondary" onClick={onReload}>
            <RefreshCw className={iconClassName} />
            Reload
          </Button>
        </div>
      </div>
    </div>
  )
}

type SourceListEditorProps = {
  title: string
  values: string[]
  placeholder: string
  onChange: (values: string[]) => void
  onAdd: (path: string) => void
  onBrowse: () => Promise<void>
}

function SourceListEditor({
  title,
  values,
  placeholder,
  onChange,
  onAdd,
  onBrowse,
}: SourceListEditorProps) {
  const [newPath, setNewPath] = useState('')

  function updateAt(index: number, value: string) {
    onChange(values.map((current, currentIndex) => (currentIndex === index ? value : current)))
  }

  function removeAt(index: number) {
    onChange(values.filter((_, currentIndex) => currentIndex !== index))
  }

  function addPath() {
    onAdd(newPath)
    setNewPath('')
  }

  return (
    <section className="border-t border-sky-100 pt-2 text-xs">
      <div className="mb-1 font-semibold text-slate-700">{title}</div>
      <div className="space-y-1">
        {values.map((value, index) => (
          <div key={`${title}-${index}`} className="flex min-w-0 gap-1.5">
            <Input
              className={`min-w-0 flex-1 font-mono ${settingsInputClassName}`}
              value={value}
              onChange={(event) => updateAt(index, event.target.value)}
            />
            <Button
              variant="ghost"
              size="sm"
              className="h-7 shrink-0 px-2 shadow-none"
              aria-label={`Remove ${title} path`}
              onClick={() => removeAt(index)}
            >
              <Trash2 className="h-4 w-4" />
            </Button>
          </div>
        ))}
      </div>
      <div className="mt-1 flex min-w-0 gap-1.5">
        <Input
          className={`min-w-0 flex-1 font-mono ${settingsInputClassName}`}
          value={newPath}
          placeholder={placeholder}
          onChange={(event) => setNewPath(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              event.preventDefault()
              addPath()
            }
          }}
        />
        <Button variant="secondary" size="sm" className="shrink-0 shadow-none" onClick={addPath}>
          <Plus className={iconClassName} />
          Add
        </Button>
        <Button variant="outline" size="sm" className="shrink-0 shadow-none" onClick={() => void onBrowse()}>
          <FolderPlus className={iconClassName} />
          Browse
        </Button>
      </div>
    </section>
  )
}

function resolveDefaultWorkflow(configs: WorkflowConfig[], settings: Settings) {
  const defaultName = settings.defaults.workflow ?? 'orca.default.toml'
  return (
    configs.find((config) => config.path === defaultName || config.name === defaultName)?.path ??
    configs[0]?.path ??
    ''
  )
}

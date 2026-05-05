import { useEffect, useMemo, useState } from 'react'
import { Bot, FileCode2, FolderArchive, Play, Settings as SettingsIcon, Square } from 'lucide-react'

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

const fallbackSettings: Settings = {
  sources: {
    agents: ['agents'],
    instructions: ['instructions'],
    workflows: ['config'],
  },
  defaults: {
    workflow: 'orca.default.toml',
    artifact_dir: 'orca-runs',
    max_parallel_agents: 8,
  },
}

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
  const [settingsOpen, setSettingsOpen] = useState(true)
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
      <header className="flex h-14 items-center border-b border-slate-800 bg-slate-950/85 px-5 backdrop-blur">
        <div className="flex items-center gap-3">
          <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-orca-500 text-slate-950">
            <Bot className="h-5 w-5" />
          </div>
          <div>
            <div className="font-semibold text-slate-50">ORCA</div>
            <div className="text-xs text-slate-400">Agents orchestration platform</div>
          </div>
        </div>
        <div className="ml-auto flex items-center gap-2">
          <span className="max-w-xl truncate text-xs text-slate-400">{status}</span>
          <Button variant="outline" size="sm" onClick={() => setSettingsOpen((open) => !open)}>
            <SettingsIcon className="mr-2 h-4 w-4" />
            Settings
          </Button>
        </div>
      </header>

      <main className="grid min-h-0 flex-1 grid-cols-[360px_1fr]">
        <aside className="min-h-0 border-r border-slate-800 bg-slate-950/60 p-4">
          <Card>
            <CardHeader>
              <CardTitle>Run goal</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <label className="space-y-2 text-sm">
                <span className="text-slate-300">Goal</span>
                <Textarea value={goal} onChange={(event) => setGoal(event.target.value)} />
              </label>
              <label className="space-y-2 text-sm">
                <span className="text-slate-300">Workflow config</span>
                <select
                  className="h-10 w-full rounded-md border border-slate-800 bg-slate-950 px-3 text-sm text-slate-100"
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
              <div className="grid grid-cols-2 gap-3">
                <label className="space-y-2 text-sm">
                  <span className="text-slate-300">Artifact dir</span>
                  <Input value={artifactDir} onChange={(event) => setArtifactDir(event.target.value)} />
                </label>
                <label className="space-y-2 text-sm">
                  <span className="text-slate-300">Max agents</span>
                  <Input
                    value={maxParallelAgents}
                    onChange={(event) => setMaxParallelAgents(event.target.value)}
                  />
                </label>
              </div>
              <label className="space-y-2 text-sm">
                <span className="text-slate-300">Instruction override dir</span>
                <Input value={instructionDir} onChange={(event) => setInstructionDir(event.target.value)} />
              </label>
              <label className="flex items-center gap-2 text-sm text-slate-300">
                <input
                  type="checkbox"
                  checked={approveGoldenPlan}
                  onChange={(event) => setApproveGoldenPlan(event.target.checked)}
                />
                Approve golden plan
              </label>
              <div className="flex gap-2">
                <Button className="flex-1" disabled={Boolean(activeRunId)} onClick={handleStartRun}>
                  <Play className="mr-2 h-4 w-4" />
                  Run
                </Button>
                <Button
                  className="flex-1"
                  variant="secondary"
                  disabled={!activeRunId}
                  onClick={handleStopRun}
                >
                  <Square className="mr-2 h-4 w-4" />
                  Stop
                </Button>
              </div>
              <Button variant="outline" className="w-full" onClick={handleValidateWorkflow}>
                Validate workflow
              </Button>
            </CardContent>
          </Card>
        </aside>

        <section className="min-h-0 overflow-hidden p-4">
          <div className="grid h-full gap-4 lg:grid-cols-[1fr_360px]">
            <Card className="min-h-0">
              <CardHeader>
                <CardTitle>Live agents</CardTitle>
              </CardHeader>
              <CardContent className="grid max-h-[calc(100vh-10rem)] gap-3 overflow-auto lg:grid-cols-2">
                {paneList.length === 0 ? (
                  <div className="rounded-lg border border-dashed border-slate-800 p-8 text-center text-slate-500">
                    Start a run to stream agent output.
                  </div>
                ) : (
                  paneList.map((pane) => <AgentPaneView key={pane.id} pane={pane} />)
                )}
              </CardContent>
            </Card>

            <div className="grid min-h-0 gap-4">
              {settingsOpen ? (
                <SettingsPanel
                  settings={settings}
                  settingsPath={settingsPath}
                  setSettings={setSettings}
                  setSettingsPath={setSettingsPath}
                  onSave={handleSaveSettings}
                  onReload={loadInitialSettings}
                />
              ) : null}
              <WorkflowEditor
                selectedWorkflow={selectedWorkflow}
                workflowContent={workflowContent}
                setWorkflowContent={setWorkflowContent}
                onLoad={handleLoadWorkflow}
                onSave={handleSaveWorkflow}
                onCreate={handleNewWorkflow}
                onDuplicate={handleDuplicateWorkflow}
                onValidate={handleValidateWorkflow}
                newWorkflowPath={newWorkflowPath}
                setNewWorkflowPath={setNewWorkflowPath}
              />
              <ArtifactBrowser
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
            </div>
          </div>
        </section>
      </main>
    </div>
  )
}

function AgentPaneView({ pane }: { pane: AgentPane }) {
  return (
    <div className="min-h-72 rounded-lg border border-slate-800 bg-slate-950">
      <div className="flex items-center justify-between border-b border-slate-800 px-3 py-2">
        <div>
          <div className="text-sm font-semibold text-slate-100">{pane.id}</div>
          <div className="text-xs text-slate-500">
            {pane.label} · {pane.model ?? 'default'}
          </div>
        </div>
        <span className="rounded-full bg-slate-900 px-2 py-1 text-xs text-slate-300">{pane.status}</span>
      </div>
      <pre className="max-h-80 overflow-auto whitespace-pre-wrap p-3 text-xs leading-5 text-slate-300">
        {pane.lines.join('\n')}
      </pre>
    </div>
  )
}

type WorkflowEditorProps = {
  selectedWorkflow: string
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

function WorkflowEditor({
  selectedWorkflow,
  workflowContent,
  newWorkflowPath,
  setWorkflowContent,
  setNewWorkflowPath,
  onLoad,
  onSave,
  onCreate,
  onDuplicate,
  onValidate,
}: WorkflowEditorProps) {
  return (
    <Card className="min-h-0">
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <FileCode2 className="h-5 w-5 text-orca-400" />
          Workflow editor
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-3">
        <div className="truncate text-xs text-slate-500">{selectedWorkflow || 'No workflow selected'}</div>
        <Textarea
          className="min-h-64 font-mono text-xs"
          value={workflowContent}
          onChange={(event) => setWorkflowContent(event.target.value)}
          placeholder="Load a workflow config to edit TOML/YAML."
        />
        <label className="space-y-2 text-sm">
          <span className="text-slate-300">New or duplicate path</span>
          <Input
            value={newWorkflowPath}
            onChange={(event) => setNewWorkflowPath(event.target.value)}
            placeholder="config/new-workflow.toml"
          />
        </label>
        <div className="flex gap-2">
          <Button className="flex-1" variant="secondary" onClick={onLoad}>
            Load
          </Button>
          <Button className="flex-1" onClick={onSave}>
            Save
          </Button>
          <Button className="flex-1" variant="secondary" onClick={onCreate}>
            New
          </Button>
        </div>
        <div className="flex gap-2">
          <Button className="flex-1" variant="secondary" onClick={onDuplicate}>
            Duplicate
          </Button>
          <Button className="flex-1" variant="outline" onClick={onValidate}>
            Validate
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}

type ArtifactBrowserProps = {
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

function ArtifactBrowser({
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
}: ArtifactBrowserProps) {
  return (
    <Card className="min-h-0">
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <FolderArchive className="h-5 w-5 text-orca-400" />
          Artifacts
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-3">
        <div className="flex gap-2">
          <Button className="flex-1" variant="secondary" onClick={onLoadArtifacts}>
            Runs
          </Button>
          <Button className="flex-1" variant="secondary" onClick={onListArtifactFiles}>
            Files
          </Button>
          <Button className="flex-1" variant="outline" onClick={onReadArtifact}>
            Read
          </Button>
        </div>
        <select
          className="h-10 w-full rounded-md border border-slate-800 bg-slate-950 px-3 text-sm text-slate-100"
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
          className="h-10 w-full rounded-md border border-slate-800 bg-slate-950 px-3 text-sm text-slate-100"
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
        <pre className="max-h-72 overflow-auto whitespace-pre-wrap rounded-md border border-slate-800 bg-slate-950 p-3 text-xs text-slate-300">
          {artifactContent || 'Artifact manifest content will appear here.'}
        </pre>
      </CardContent>
    </Card>
  )
}

type SettingsPanelProps = {
  settings: Settings
  settingsPath: string
  setSettings: (settings: Settings) => void
  setSettingsPath: (path: string) => void
  onSave: () => void
  onReload: () => void
}

function SettingsPanel({
  settings,
  settingsPath,
  setSettings,
  setSettingsPath,
  onSave,
  onReload,
}: SettingsPanelProps) {
  function updateSources(key: keyof Settings['sources'], value: string) {
    setSettings({
      ...settings,
      sources: {
        ...settings.sources,
        [key]: value
          .split('\n')
          .map((line) => line.trim())
          .filter(Boolean),
      },
    })
  }

  return (
    <Card className="min-h-0">
      <CardHeader>
        <CardTitle>Settings</CardTitle>
      </CardHeader>
      <CardContent className="max-h-[calc(100vh-10rem)] space-y-4 overflow-auto">
        <label className="space-y-2 text-sm">
          <span className="text-slate-300">Settings file</span>
          <Input value={settingsPath} onChange={(event) => setSettingsPath(event.target.value)} />
        </label>
        <label className="space-y-2 text-sm">
          <span className="text-slate-300">Agent sources</span>
          <Textarea
            value={settings.sources.agents.join('\n')}
            onChange={(event) => updateSources('agents', event.target.value)}
          />
        </label>
        <label className="space-y-2 text-sm">
          <span className="text-slate-300">Instruction sources</span>
          <Textarea
            value={settings.sources.instructions.join('\n')}
            onChange={(event) => updateSources('instructions', event.target.value)}
          />
        </label>
        <label className="space-y-2 text-sm">
          <span className="text-slate-300">Workflow sources</span>
          <Textarea
            value={settings.sources.workflows.join('\n')}
            onChange={(event) => updateSources('workflows', event.target.value)}
          />
        </label>
        <label className="space-y-2 text-sm">
          <span className="text-slate-300">Default workflow</span>
          <Input
            value={settings.defaults.workflow ?? ''}
            onChange={(event) =>
              setSettings({
                ...settings,
                defaults: { ...settings.defaults, workflow: event.target.value || null },
              })
            }
          />
        </label>
        <label className="space-y-2 text-sm">
          <span className="text-slate-300">Default artifact dir</span>
          <Input
            value={settings.defaults.artifact_dir ?? ''}
            onChange={(event) =>
              setSettings({
                ...settings,
                defaults: { ...settings.defaults, artifact_dir: event.target.value || null },
              })
            }
          />
        </label>
        <label className="space-y-2 text-sm">
          <span className="text-slate-300">Default max parallel agents</span>
          <Input
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
        <div className="flex gap-2">
          <Button className="flex-1" onClick={onSave}>
            Save
          </Button>
          <Button className="flex-1" variant="secondary" onClick={onReload}>
            Reload
          </Button>
        </div>
      </CardContent>
    </Card>
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

# ORCA workflow model

ORCA has two execution models:

1. **Command workflow**: `orca run --config config/orca.default.toml` runs explicitly configured commands with dependencies, bounded concurrency, retries, timeouts, and fail-fast/continue-on-error behavior.
2. **Goal workflow**: `orca "..."` or `orca goal --goal "..."` runs the configurable multi-agent loop documented here.

## Goal workflow overview

The goal workflow is driven by `[orchestration]` config. `GoalOrchestrator` loads TOML/YAML config, validates nodes/backends/connections, materializes the default workflow when no config exists, and then runs each iteration with the shared agent backend and output-event model.

Execution is graph-based:

- `[[orchestration.connections]]` and per-node `depends_on` define **scheduling dependencies only**.
- Per-node `inputs` define the ordered context injected into that node prompt.
- A node can depend on one upstream node and consume input from a different, farther upstream node.
- A connection does not inject context unless the source also appears in the target node's `inputs`.

```text
User goal
  |
  v
Configured graph nodes run when dependencies are complete
  |
  v
Golden implementation plan output_contract = "golden_plan"
  |
  +-- manual approval missing --> stop
  |
  v
Implementation/test/KPI/coverage nodes
  |
  v
Completion contracts:
  - output_contract = "kpi" passes, if configured
  - output_contract = "test" passes
  - output_contract = "coverage" signs off
  |
  +-- all pass --> goal complete
  |
  +-- any fail --> feedback nodes produce next-iteration feedback
```

## Default node set

When no `[orchestration]` config is supplied, ORCA uses this default node set with the current default backend profile:

| Output contract | Default nodes |
| --- | --- |
| `feature` | `feature-generator` |
| `plan` | `planner-a`, `planner-b`, `planner-c`, `planner-d` |
| `critique` | `critic-a`, `critic-b`, `critic-c`, `critic-d` |
| `golden_plan` | `orchestrator` |
| `kpi` | `kpi-agent`, `kpi-measure-agent` |
| `test_plan` | `test-planner-a`, `test-planner-b`, `test-planner-c`, `test-planner-d`, `golden-test-plan` |
| `test` | `test-agent` |
| `implementation` | `work-agent` |
| `coverage` | `coverage-agent` |
| `feedback` | `feedback-agent` |

Default goal settings:

| Setting | Default |
| --- | --- |
| `max_parallel_agents` | `8` |
| `min_successful_planners` | `2` |
| `min_successful_critics` | `1` |
| `max_iterations` | `10` |
| `approval_mode` | `auto` |
| Agent command | `copilot` |
| Agent timeout | `900` seconds |

## Approval behavior

The first `golden_plan` output is the workflow boundary between analysis/planning and code/test execution.

- `approval_mode = "auto"` lets the workflow continue automatically.
- `approval_mode = "manual"` requires `--approve-golden-plan`.
- If approval is missing, ORCA stops after generating the golden plan and reports feedback: `golden plan requires approval`.

## Orchestration config model

Goal config is graph-shaped and can be written in TOML (`config/orca.default.toml`) or YAML (`config/orca.yaml` / `config/orca.yml`). The CLI defaults to the `config/` folder and accepts an explicit path with `--config` or `--config-file`.

`settings.toml` controls where ORCA looks for reusable setup files. ORCA searches `./settings.toml` first and then `~/.config/orca/settings.toml`, or an explicit file passed with `--settings`.

```toml
[sources]
agents = ["agents"]
instructions = ["instructions"]
workflows = ["config"]

[defaults]
workflow = "orca.default.toml"
artifact_dir = "orca-runs"
max_parallel_agents = 8
```

Settings precedence is explicit run fields, then `settings.toml`, then workflow config values, then embedded defaults. Workflow config names are resolved through ordered `[sources].workflows`; instruction names are resolved through ordered `[sources].instructions` before embedded instructions. Agent source directories are validated and persisted in settings, while concrete external agent-definition loading remains workflow-config based for now.

| Config area | Purpose |
| --- | --- |
| `[orchestration]` | Global workflow settings such as artifact directory, instruction directory, approval mode, iteration count, and max parallel agents. |
| `[orchestration.defaults]` | Default backend, model, timeout, retry, and retry delay values inherited by nodes. |
| `[[orchestration.backends]]` | AI CLI command templates. A backend profile has an `id`, `program`, and templated `args`. |
| `[[orchestration.nodes]]` | Executable nodes with stable IDs, kind, output contract, instruction, optional backend/model/command overrides, resource locks, dependencies, and ordered input selectors. |
| `[[orchestration.connections]]` | Scheduling-only one-to-many dependency edges. |

Backend and node args support these templates:

| Template | Replaced with |
| --- | --- |
| `{goal}` | User goal text. |
| `{context}` | Ordered rendered input bundle for the current node. |
| `{instructions}` | Resolved embedded or override Markdown instructions. |
| `{prompt}` | Default prompt containing instructions, goal, and context. |
| `{model}` | Configured model when used by backend templates. |

Legacy `[agents]` config and node `role` fields are intentionally not supported.

## Completion gates

ORCA considers a goal complete only when all configured completion contracts pass:

| Contract | Expected signal |
| --- | --- |
| `kpi` | `passed: true`; optional when no KPI outputs exist. |
| `test` | `passed: true`; required. |
| `coverage` | `signed_off: true`; required. |

If any required gate is missing or false, the iteration is incomplete. Gate feedback is collected and, when feedback nodes are configured, converted into next-iteration feedback.

## RTL spec-to-environment workflow

`config/rtl-spec-to-env.toml` is a specialized SystemVerilog workflow for taking a specification to RTL plus verification environment:

| Agent | Output contract | Scope |
| --- | --- | --- |
| `rtl-design-agent` | `implementation` | Implements synthesizable SystemVerilog RTL only. It must not edit UVM, testbench, or stimulus code. |
| `uvm-test-bench-agent` | `test` | Implements UVM/SystemVerilog testbench and stimulus only. It must not edit RTL design implementation. |

This config removes KPI generation and measurement nodes. The final signoff uses `rtl-env-review-agent` as the `coverage` contract over the generated RTL and verification environment.

## Parallelism model

The graph executor runs ready nodes concurrently up to `max_parallel_agents`. Readiness is determined by scheduling dependencies only. Optional `resources` locks prevent nodes that share a resource, such as `workspace-writer`, from running at the same time.

Inputs are modeled as ordered bundles. Every injected source is rendered with a simple source heading followed by the source content:

```text
## planner-a

<planner-a output>

## planner-b

<planner-b output>
```

## Artifacts and live UI

Every goal run creates an artifact workspace containing:

- `manifest.json`: final `GoalSummary`.
- `events.jsonl`: workflow and agent-input events.
- `iteration-N/...`: per-iteration node outputs and completion reports.
- `iteration-N/feedback.md`: retry feedback when an iteration fails and another iteration can run.

Output JSON and artifact metadata use node IDs, node kind, output contract, configured artifact directory, and optional phase labels. They do not include role fields.

The TUI and Tauri/React desktop app consume the same output-event model:

- `PhaseStarted`
- `AgentStarted`
- `AgentInput`
- `Line`
- `AgentFinished`
- `IterationSummary`
- `Shutdown`

This keeps terminal rendering and GUI rendering separate while sharing the same workflow state model. The product GUI uses Tauri commands/events to bridge `orca-core` into the React/TypeScript frontend under `ui/desktop`.

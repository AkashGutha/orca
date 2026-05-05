# ORCA - Agents orchestration platform

`orca` is a Rust CLI foundation for orchestrating external commands and AI agent harnesses with bounded concurrency, dependency ordering, retries, timeouts, cancellation, and structured output. It is designed to support configurable open-source agent backends, with Copilot CLI available as one backend profile.

## Usage

Create `config/orca.default.toml`:

```toml
max_concurrency = 4
continue_on_error = false

[[commands]]
id = "build"
program = "cargo"
args = ["build"]
timeout_secs = 120

[[commands]]
id = "test"
program = "cargo"
args = ["test"]
depends_on = ["build"]
timeout_secs = 180
retries = 1
retry_delay_ms = 500
```

Run the configured commands:

```sh
cargo run -p orca-cli -- run
```

Validate a config without running anything:

```sh
cargo run -p orca-cli -- validate-config
```

List command IDs:

```sh
cargo run -p orca-cli -- list
```

Emit structured JSON logs and a JSON summary:

```sh
cargo run -p orca-cli -- run --json
```

Run a goal-oriented agent workflow. ORCA is goal-first, so positional text is treated as the goal and no config file is required for the default agent flow:

```sh
cargo run -p orca-cli -- "Implement feature X"
```

You can also use explicit goal flags:

```sh
cargo run -p orca-cli -- goal --goal "Implement feature X" --config config/orca.default.toml --approve-golden-plan
```

ORCA shows live agent output with an interactive terminal UI when stdout is a terminal. Parallel agents render as side-by-side wrapping panes headed by node ID, output contract, and status, with a controls footer pinned to the bottom at all times. At the end of each iteration, the TUI clears the agent panes and shows a short summary of what happened plus whether another iteration is starting and why. Press `q`, `Esc`, or `Ctrl-C` in the TUI to request cancellation; running child agent processes are terminated. If the TUI is not behaving well in a terminal, rerun with `--plain` for the fallback line-oriented renderer. Use `--tui` to request the live UI or `--json` for machine-readable output:

```sh
cargo run -p orca-cli -- "Implement feature X" --plain
cargo run -p orca-cli -- "Implement feature X" --tui
cargo run -p orca-cli -- "Implement feature X" --json
```

The product GUI is the Tauri + React desktop app:

```sh
npm install --prefix ui/desktop
npm run dev --prefix ui/desktop
cargo run --manifest-path crates/orca-desktop-runtime/Cargo.toml
```

Or use the frontend helper:

```sh
npm run tauri:dev --prefix ui/desktop
```

Build the React frontend with:

```sh
npm run build --prefix ui/desktop
```

Build simple executables:

```sh
# CLI
cargo build -p orca-cli --release
./target/release/orca

# GUI
npm install --prefix ui/desktop
npm run build --prefix ui/desktop
cargo build --manifest-path crates/orca-desktop-runtime/Cargo.toml --release
./crates/orca-desktop-runtime/target/release/orca-desktop
```

The Tauri runtime requires the platform WebView dependencies. On Linux, install a recent WebKitGTK/GLib stack before running the `orca-desktop-runtime` manifest; the default Rust workspace excludes that runtime crate so `cargo build --workspace --all-targets --all-features` stays portable on older build images.

The workspace `orca-desktop` binary at `./target/debug/orca-desktop` is only a portable launcher stub that prints these commands. The real Tauri app executable is built from `crates/orca-desktop-runtime`.

The React desktop app includes settings editing, workflow config discovery from `settings.toml`, workflow config editing/validation, goal run controls, live multi-agent output panes, and artifact browsing.

Or read the goal from Markdown:

```sh
cargo run -p orca-cli -- goal --goal-file goal.md --config config/orca.default.toml --approve-golden-plan
```

## Command options

Each command supports:

- `id`: unique command identifier.
- `program`: executable to run directly without a shell.
- `args`: argument list.
- `depends_on`: command IDs that must finish successfully first.
- `env`: per-command environment variables.
- `cwd`: per-command working directory.
- `timeout_secs`: command timeout.
- `retries`: retry count after the first attempt fails.
- `retry_delay_ms`: delay between retry attempts.

## Failure behavior

By default, `orca` stops scheduling new commands and cancels running commands after the first failure. Use `continue_on_error = true` in config or `--continue-on-error` on the CLI to keep running independent commands. Commands whose dependencies fail are skipped.

The process exits with a non-zero code if any command fails, times out, is cancelled, or is skipped due to failed dependencies.

## Goal agent orchestration

The `goal` command coordinates configurable agents around a user goal:

1. Runs optional feature-generation agents to expand the goal into feature context.
2. Runs multiple planning agents in parallel with generated feature context and any prior feedback.
3. Runs critique agents one-to-one against the planning outputs.
4. Runs an orchestration agent to synthesize a golden plan.
5. Pauses at a manual approval gate unless `approval_mode = "auto"` or `--approve-golden-plan` is provided.
6. Runs multiple test-planning agents in parallel from the golden implementation plan.
7. Runs an orchestration agent over those test-planning outputs to create one golden test plan.
8. Runs the singleton test-generation agent from the golden test plan in parallel with KPI generation, the singleton work agent, and any parallel-run agents from the golden implementation plan. The test agent only writes tests; the work agent does the implementation work, and neither consumes the other's output.
9. Runs the final measurement phase with feature-coverage and KPI-measurement agents in parallel after work agents have completed.
10. If tests pass, KPI measurement passes, and feature coverage signs off, stops.
11. If feature coverage, KPI measurement, tests, or another gate fails, runs feedback agents to summarize gate feedback for the next planning pass.
12. Feeds feedback-agent output back through feature generation and planning for the next iteration until `max_iterations` is reached.

Rust models the agent primitive with a trait and shared configuration rather than class inheritance. Each agent is an `orchestration.nodes` entry with a node kind, output contract, backend profile, command template, ordered inputs, and Markdown instructions. Connections/dependencies schedule nodes; only the ordered `inputs` list injects upstream content into a node prompt.

Config is optional. Add `config/orca.default.toml`, `config/orca.yaml`, or `config/orca.yml` only when you want to override the default agents, artifact directory, instruction directory, backend command templates, parallelism, or iteration limits. The CLI defaults to `config/orca.default.toml` and also discovers `config/orca.yaml` / `config/orca.yml` when the default TOML file is absent. Use `--config <CONFIG_FILE>` or `--config-file <CONFIG_FILE>` to pass an explicit file path. Legacy `[agents]` configs are rejected; use `[orchestration]` with `[[orchestration.nodes]]`.

For a detailed phase-by-phase model of the goal workflow, default agents, completion gates, artifacts, and UI event flow, see [WORKFLOW.md](WORKFLOW.md).

### Copilot CLI helper skill

ORCA includes a repo-owned Copilot CLI helper skill at `skills/copilot-cli-helper/SKILL.md` for guidance about correct Copilot CLI invocation, permissions, sessions, models, and troubleshooting. It is not part of the default active agent mix right now; add a `copilot_cli_helper` spec only if you want to experiment with it in a custom flow.

The core ORCA backend invocation is:

```sh
copilot --silent --no-ask-user -p "..."
```

The current default backend profile invokes Copilot CLI through command templates. Analysis and review nodes use the low-permission non-interactive form above. Test and implementation nodes add `--allow-all-tools` because they may need to edit files or run validation commands; implementation also explicitly uses `--model gpt-5.5`. The default flow fans out to 4 planning nodes, 4 critique nodes, and 4 test-planning nodes, while workspace-writing nodes share a `workspace-writer` resource lock so they do not edit concurrently. `max_parallel_agents = 8` still applies to nodes whose dependencies and resources are available. For other trusted open-source AI agent harnesses, override backend command templates in `[orchestration.backends]` while keeping the same workflow graph.

Example orchestration config:

```toml
[orchestration]
artifact_dir = "orca-runs"
instruction_dir = "agents"
max_parallel_agents = 8
min_successful_planners = 2
min_successful_critics = 1
approval_mode = "manual"
max_iterations = 10

[orchestration.defaults]
backend = "copilot"
model = "default"
timeout_secs = 900

[[orchestration.backends]]
id = "copilot"
program = "copilot"
args = ["--silent", "--no-ask-user", "-p", "{prompt}"]

[[orchestration.nodes]]
id = "feature-generator"
output_contract = "feature"
instruction = "feature_generation.md"
inputs = [{ source = "feedback" }]

[[orchestration.nodes]]
id = "planner-a"
output_contract = "plan"
instruction = "planning.md"
timeout_secs = 600
retries = 1
inputs = [{ source = "feedback" }, { source = "feature-generator" }]

[[orchestration.nodes]]
id = "planner-b"
output_contract = "plan"
instruction = "planning.md"
inputs = [{ source = "feedback" }, { source = "feature-generator" }]

[[orchestration.nodes]]
id = "critic-a"
output_contract = "critique"
instruction = "critique.md"
inputs = [{ source = "planner-a" }]

[[orchestration.nodes]]
id = "critic-b"
output_contract = "critique"
instruction = "critique.md"
inputs = [{ source = "planner-b" }]

[[orchestration.nodes]]
id = "orchestrator"
output_contract = "golden_plan"
instruction = "orchestration.md"
inputs = [
  { source = "feedback" },
  { source = "planner-a" },
  { source = "planner-b" },
  { source = "critic-a" },
  { source = "critic-b" },
]

[[orchestration.nodes]]
id = "kpi-agent"
output_contract = "kpi"
instruction = "kpi_generation.md"
inputs = [{ source = "orchestrator" }]

[[orchestration.nodes]]
id = "kpi-measure-agent"
output_contract = "kpi"
instruction = "kpi_measurement.md"
inputs = [{ source = "orchestrator" }, { source = "work-agent" }]

[[orchestration.nodes]]
id = "test-planner-a"
output_contract = "test_plan"
instruction = "test_planning.md"
inputs = [{ source = "orchestrator" }]

[[orchestration.nodes]]
id = "test-agent"
output_contract = "test"
instruction = "test_generation.md"
args = ["--silent", "--no-ask-user", "--allow-all-tools", "-p", "{prompt}"]
inputs = [{ source = "golden-test-plan" }]

[[orchestration.nodes]]
id = "work-agent"
output_contract = "implementation"
instruction = "work.md"
model = "gpt-5.5"
args = [
  "--silent",
  "--no-ask-user",
  "--model",
  "gpt-5.5",
  "--allow-all-tools",
  "-p",
  "{prompt}",
]
inputs = [{ source = "orchestrator" }]

[[orchestration.nodes]]
id = "coverage-agent"
output_contract = "coverage"
instruction = "feature_coverage.md"
inputs = [{ source = "orchestrator" }, { source = "work-agent" }]

[[orchestration.nodes]]
id = "feedback-agent"
output_contract = "feedback"
instruction = "feedback.md"
inputs = [{ source = "gate-feedback" }]

[[orchestration.connections]]
from = "planner-a"
to = ["critic-a", "orchestrator"]

[[orchestration.connections]]
from = "planner-b"
to = ["critic-b", "orchestrator"]
```

See [`config/orca.default.toml`](config/orca.default.toml) for the generated default workflow expressed as a config file.

## Settings

ORCA loads `settings.toml` from the workspace first, then falls back to a user config at `~/.config/orca/settings.toml` when the workspace file is missing. Use `--settings path/to/settings.toml` to load a specific settings file.

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

Source directories are ordered. Explicit CLI/GUI run fields win first, then `settings.toml`, then workflow config values, then embedded ORCA defaults. Workflow names such as `orca.default.toml` are resolved through `[sources].workflows`; instruction names are resolved through `[sources].instructions` before embedded instructions. Agent source directories are stored and validated now so ORCA can grow external agent definitions without changing the settings format.

Examples:

```sh
cargo run -p orca-cli -- "Implement feature X" --settings settings.toml
cargo run -p orca-cli -- run --settings settings.toml
cargo run -p orca-cli -- validate-config --settings settings.toml
```

The GUI has a **Settings** button on the far right of the top bar. It opens a right-side panel for editing source directories and run defaults, loading a settings file, saving it back to TOML, and applying defaults to the run controls.

For RTL spec-to-environment work, use [`config/rtl-spec-to-env.toml`](config/rtl-spec-to-env.toml):

```sh
cargo run -p orca-cli -- goal --goal-file spec.md --config config/rtl-spec-to-env.toml
```

That workflow removes KPI agents and replaces the default implementation agents with:

| Agent | Output contract | Scope |
| --- | --- | --- |
| `rtl-design-agent` | `implementation` | Synthesizable SystemVerilog RTL design only. It must not edit UVM/testbench/stimulus code. |
| `uvm-test-bench-agent` | `test` | UVM/SystemVerilog testbench and stimulus only. It must not edit RTL design implementation. |

Backend and node command arguments support simple template replacement:

- `{goal}`: goal text
- `{context}`: phase context from prior agent outputs
- `{instructions}`: resolved Markdown instruction text
- `{prompt}`: the full default prompt containing instructions, goal, and context
- `{model}`: configured model value when used in backend templates

Test-generation and KPI-measurement agents should return JSON with a pass/fail result:

```json
{ "passed": true, "feedback": "" }
```

KPI-generation agents define measurable KPI targets; KPI-measurement agents run in the final measurement phase and decide whether those targets were actually met. When KPI-measurement outputs are present, they are the KPI completion gate. Workflows without KPI-generation or KPI-measurement outputs skip the KPI gate.

The feature coverage agent should return:

```json
{
  "signed_off": true,
  "covered_features": ["feature A"],
  "missing_features": [],
  "feedback": ""
}
```

Goal runs write artifacts under `artifact_dir` with one directory per iteration:

- `iteration-1/feature-generation/*.md`
- `iteration-1/planning/*.md`
- `iteration-1/critique/*.md`
- `iteration-1/golden-plan/*.md`
- `iteration-1/kpi/*.md`
- `iteration-1/test-planning/*.md`
- `iteration-1/golden-test-plan/*.md`
- `iteration-1/tests/*.md`
- `iteration-1/work/*.md`
- `iteration-1/kpi-measurement/*.md`
- `iteration-1/feature-coverage/*.md`
- `iteration-1/feedback/*.md` when completion gates fail
- `manifest.json`
- `events.jsonl` with full per-agent input records before each invocation. The TUI and plain renderer show a short input glimpse for each agent.

## Developer commands

```sh
cargo fmt
cargo clippy --all-targets --all-features
cargo test --all
```

## React calculator app

This repository also includes a lightweight frontend-only React calculator app
under `web/`. It provides Basic, Scientific, Unit Converter, and Number Systems
modes without adding UI, math, parser, or conversion libraries.

```sh
npm install
npm run dev
npm run build
```

The app is served by Vite from `web/index.html`; source files live in
`web/src/`.

## React calculator app

A lightweight frontend-only React calculator suite lives in `web/`. It uses React, React DOM, and Vite build tooling only.

```sh
cd web
npm install
npm run dev
npm run build
```

The app includes Basic, Scientific, Unit Converter, and Number-System Converter modes.

## 32-bit pipelined ALU example

The `verilog_alu_8bit/` directory name is historical. Its primary RTL is now `rtl/alu_32bit_pipelined.v`, a 32-bit ALU with synchronous active-low reset, valid/bubble pipeline control, the documented opcode map, full result flags, and exact five-cycle latency from input sample to public output. The legacy `rtl/alu_8bit.v` and `tb/tb_alu_8bit.v` files are retained only as historical references.

Run the primary self-checking ALU validation with:

```sh
cd verilog_alu_8bit
mkdir -p sim
iverilog -g2012 -Wall -o sim/tb_alu_32bit_pipelined.vvp \
  rtl/alu_32bit_pipelined.v tb/tb_alu_32bit_pipelined.v
vvp sim/tb_alu_32bit_pipelined.vvp
```

The expected final summary is:

```text
PASS: all 32-bit pipelined ALU tests passed
```

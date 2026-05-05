---
name: copilot-cli-helper
description: Help configure and use GitHub Copilot CLI, especially as ORCA's non-interactive agent backend.
---

# Copilot CLI Helper

Use this skill when the task involves GitHub Copilot CLI command usage, ORCA Copilot backend configuration, non-interactive agent invocation, permission flags, session/model options, filesystem or URL access, or troubleshooting Copilot CLI calls launched by ORCA.

## Non-interactive prompts

Use `-p` or `--prompt` when Copilot CLI must run as an automation step and exit after producing a result:

```sh
copilot --silent --no-ask-user -p "<instructions + goal + context>"
```

ORCA's default agent backend should invoke Copilot CLI as:

```sh
copilot --silent --no-ask-user -p "<agent prompt>"
```

Do not pass the prompt as a positional argument. Copilot CLI reports `Invalid command format` and suggests `-i` or `-p` when a prompt is passed positionally.

## Permissions and safety

Permission flags are powerful. Recommend the narrowest flag set that satisfies the workflow and put permission flags before `-p`.

Start with no broad tool access for analysis-only work:

```sh
copilot --silent --no-ask-user -p "<prompt>"
```

Allow file editing when the task must write files:

```sh
copilot --silent --no-ask-user --allow-tool='write' -p "<prompt>"
```

Allow only a scoped shell command family when validation is required:

```sh
copilot --silent --no-ask-user --allow-tool='shell(cargo test:*)' -p "<prompt>"
```

Allow git commands while blocking pushes:

```sh
copilot --silent --no-ask-user --allow-tool='shell(git:*)' --deny-tool='shell(git push)' -p "<prompt>"
```

Treat `--allow-all-tools`, `--allow-all`, and `--yolo` as trusted-workflow-only options. Use them only when broad tool access risk has been explicitly accepted:

```sh
copilot --silent --no-ask-user --allow-all-tools -p "<prompt>"
copilot --silent --no-ask-user --allow-all -p "<prompt>"
copilot --silent --no-ask-user --yolo -p "<prompt>"
```

## Model selection

Start Copilot CLI with a specific model:

```sh
copilot --silent --no-ask-user --model gpt-5.2 -p "<prompt>"
```

For ORCA agent overrides, add model flags to the agent `args` before `-p` only when overriding the default argument construction.

## Reasoning and output visibility

Use reasoning flags when the workflow needs more reasoning effort or visible reasoning summaries. Copilot CLI can request reasoning summaries, but it does not expose raw private chain-of-thought:

```sh
copilot --silent --no-ask-user --reasoning-effort high -p "<prompt>"
copilot --silent --no-ask-user --model gpt-5.5 --reasoning-effort high --enable-reasoning-summaries -p "<prompt>"
```

Tool permissions and tool-call visibility are different concerns. `--allow-tool` and `--allow-all-tools` permit tool use; `--silent` intentionally keeps output simple for scripting. If the caller wants to observe richer CLI events, remove `--silent` or use JSONL output:

```sh
copilot --no-ask-user --allow-all-tools -p "<prompt>"
copilot --no-ask-user --allow-all-tools --output-format json -p "<prompt>"
```

## Session management

Resume or name sessions when continuity matters:

```sh
copilot --silent --no-ask-user --continue -p "<prompt>"
copilot --silent --no-ask-user --resume -p "<prompt>"
copilot --silent --no-ask-user --resume=<session-id> -p "<prompt>"
copilot --silent --no-ask-user --resume="my feature" -p "<prompt>"
copilot --silent --no-ask-user --resume=0cb916d -p "<prompt>"
copilot --silent --no-ask-user --resume=0cb916db-26aa-40f2-86b5-1ba81b225fd2 -p "<prompt>"
copilot --silent --no-ask-user --name="my feature" -p "<prompt>"
```

Resume with broad tool approval only for a trusted workflow:

```sh
copilot --silent --no-ask-user --allow-all-tools --resume -p "<prompt>"
```

## Filesystem and URL access

Allow access to an additional directory:

```sh
copilot --silent --no-ask-user --add-dir /home/user/projects -p "<prompt>"
```

Allow multiple directories:

```sh
copilot --silent --no-ask-user --add-dir ~/workspace --add-dir /tmp -p "<prompt>"
```

Allow GitHub API access:

```sh
copilot --silent --no-ask-user --allow-url=github.com -p "<prompt>"
```

Deny a domain:

```sh
copilot --silent --no-ask-user --deny-url=https://malicious-site.com -p "<prompt>"
copilot --silent --no-ask-user --deny-url=malicious-site.com -p "<prompt>"
```

Prefer `--add-dir <path>` over `--allow-all-paths`, and prefer `--allow-url=<domain>` over `--allow-all-urls`. Use unrestricted path or URL flags only in trusted workflows that truly require them and explicitly accept the risk.

## Tool allow/deny examples

Allow file editing:

```sh
copilot --silent --no-ask-user --allow-tool='write' -p "<prompt>"
```

Allow only cargo test commands:

```sh
copilot --silent --no-ask-user --allow-tool='shell(cargo test:*)' -p "<prompt>"
```

Allow all git commands except `git push`:

```sh
copilot --silent --no-ask-user --allow-tool='shell(git:*)' --deny-tool='shell(git push)' -p "<prompt>"
```

Allow all but one specific tool from an MCP server named `MyMCP`:

```sh
copilot --silent --no-ask-user --deny-tool='MyMCP(denied_tool)' --allow-tool='MyMCP' -p "<prompt>"
```

## Repository initialization

Initialize Copilot instructions for a repository:

```sh
copilot init
```

## Recommended ORCA agent backend settings

Default ORCA agent calls should use:

```sh
copilot --silent --no-ask-user -p "<instructions + goal + context>"
```

ORCA's built-in defaults are role-aware. Some existing autonomous defaults use broad tool approval because those roles are intended to run trusted repo work; that is current automation behavior, not the safest general recommendation, and it assumes the caller accepts the risk.

| Role | Default flags | Reason |
| --- | --- | --- |
| `planning`, `critique`, `orchestration`, `kpi_generation`, `feature_coverage`, `feedback`, `copilot_cli_helper` | `--silent --no-ask-user -p "<prompt>"` | Non-interactive analysis without broad tool approval. |
| `test_generation`, `parallel_run` | `--silent --no-ask-user --allow-all-tools -p "<prompt>"` | Trusted autonomous roles that may need to run validation commands. |
| `work` | `--silent --no-ask-user --model gpt-5.5 --allow-all-tools -p "<prompt>"` | Trusted autonomous implementation role pinned to GPT-5.5. |

For trusted autonomous repo work, an agent override may add:

```toml
[[agents.specs]]
id = "work-agent"
role = "work"
command = "copilot"
args = [
  "--silent",
  "--no-ask-user",
  "--model",
  "gpt-5.5",
  "--reasoning-effort",
  "high",
  "--enable-reasoning-summaries",
  "--allow-all-tools",
  "-p",
  "{instructions}\n\nGoal:\n{goal}\n\nContext:\n{context}",
]
```

For a broader trusted workflow with accepted broad-access risk:

```toml
[[agents.specs]]
id = "work-agent"
role = "work"
command = "copilot"
args = [
  "--silent",
  "--no-ask-user",
  "--allow-all",
  "-p",
  "{instructions}\n\nGoal:\n{goal}\n\nContext:\n{context}",
]
```

## Troubleshooting

- `Invalid command format`: the prompt was likely passed as a positional argument. Use `-p "<prompt>"`.
- Agent hangs waiting for input: add `--no-ask-user` and ensure required permissions are pre-approved.
- Agent cannot edit: add `--allow-tool='write'`.
- Agent cannot run commands: add the narrowest needed shell permission, such as `--allow-tool='shell(cargo test:*)'`, or use `--allow-all-tools` for trusted workflows.
- Output is too minimal: add `--reasoning-effort high` and, for OpenAI/GPT models, `--enable-reasoning-summaries`; remove `--silent` or use `--output-format json` if richer CLI events are more important than stable scripting output.
- Agent cannot read a path: add `--add-dir <path>` or, for trusted workflows only, `--allow-all-paths`.
- Agent cannot access a URL: add `--allow-url=<domain>` or, for trusted workflows only, `--allow-all-urls`.

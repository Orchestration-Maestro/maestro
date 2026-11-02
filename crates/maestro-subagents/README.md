# Authored subagents

Delegate a task to a named Markdown agent in a separate Maestro process. Each
child has its own context and no persisted session. The `subagent` tool supports
one task, up to eight parallel tasks with four children at once, or a sequential
chain that substitutes `{previous}` with the preceding output.

## Build and load

From the workspace root:

```sh
cargo build --locked -p maestro-subagents --example component \
  --target wasm32-wasip2 --release
mkdir -p crates/maestro-subagents/dist
cp target/wasm32-wasip2/release/examples/component.wasm \
  crates/maestro-subagents/dist/component.wasm
maestro -e ./crates/maestro-subagents
```

If Cargo uses a custom target directory, copy the artifact from that directory.
The package manifest selects the built component and the three workflow prompts;
consumers load the built package without compiling Rust. This source checkout
needs the build above before loading it as a package.

Copy `agents/*.md` into `<agent_dir>/agents`. The default agent directory is
`~/.maestro/agent`; `MAESTRO_CODING_AGENT_DIR` can select another directory.
Copy `prompts/*.md` into `<agent_dir>/prompts` when loading the component alone
rather than installing the complete built package. The
[package guide](../../docs/package-guide.md) describes package installation.

## Definitions and trust

```markdown
---
name: scout
description: Find the code a task needs
tools: read, grep, find, ls, bash
model: claude-haiku-4-5
---
Return concise findings with exact file paths and line ranges.
```

`name` and `description` are required. `tools` is an optional comma-separated
list; `model` is an optional model selector. Omit either to use the child's
normal defaults. The body is appended to the child's system prompt.
Unreadable files and files without a name or description are skipped. Malformed
metadata and non-text optional restrictions report an error. Symbolic links to
files and agent directories are followed. Definitions are read on every call.

The default scope is `user`, reading `<agent_dir>/agents`. `project` reads the
nearest ancestor's `.maestro/agents`; `both` reads both sources and lets project
definitions replace matching user names. Interactive calls ask before running
requested project agents unless `confirmProjectAgents` is false. Headless calls
do not ask. Project prompts can direct tools: use them only in trusted repositories.

## Modes

```json
{"agent":"scout","task":"Find authentication code"}
```

```json
{"tasks":[{"agent":"scout","task":"Find models"},{"agent":"scout","task":"Find providers"}]}
```

```json
{"chain":[{"agent":"scout","task":"Find the read tool"},{"agent":"planner","task":"Suggest improvements using {previous}"}]}
```

Each task can supply `cwd`. A chain stops at the first failed child or provider
error. Cancellation signals every owned child. No extra deadline is added.
`MAESTRO_BIN_PATH` selects the child executable when supplied; otherwise the
runner invokes `maestro` on `PATH`. Children receive literal arguments, not a
shell command: `--mode json -p --no-session`, optional model/tools, an appended
role file and `Task: <instructions>`. Private role files are removed after
success, failure, cancellation or a dropped invocation.

## Views and samples

Collapsed views show recent text/tool calls, status and usage. Expanded views
show complete tasks, all tool calls and final Markdown output. Parallel updates
retain each child's progress. Usage includes turns, input/output/cache tokens,
reported cost, the latest context size and the selected or reported model.

| Agent | Purpose | Model | Tools |
| --- | --- | --- | --- |
| `scout` | Compressed codebase findings | `claude-haiku-4-5` | read, grep, find, ls, bash |
| `planner` | Implementation plan | `claude-sonnet-4-5` | read, grep, find, ls |
| `reviewer` | Quality and security review | `claude-sonnet-4-5` | read, grep, find, ls, bash |
| `worker` | General-purpose execution | `claude-sonnet-4-5` | child defaults |

The `/implement` prompt chains scout → planner → worker; `/scout-and-plan` stops
after planning; `/implement-and-review` chains worker → reviewer → worker.

## Controlled qualification

```sh
cargo test --locked -p maestro-subagents
cargo test --locked -p maestro --test integration subagent_example::
```

The native tests load the actual component with application-owned filesystem,
process and UI effects. Controlled Python children validate launch arguments,
stream JSON, report usage, fail and acknowledge cancellation without model calls.
The signal/concurrency qualification runs on Linux; file-link cases run on Unix.

---
name: workspace
description: "Inspect or control Maestro workspaces, tabs, panes and recognized agents only when the user explicitly requests workspace control. Requires MAESTRO_ENV=1."
---

# Workspace control

Use this skill only when the user asks for workspace control. A task that could
benefit from a background terminal or another agent is not permission to use it.
Skip loading this skill if it is already in your context.

Before any control command, verify that you are inside a managed pane:

```sh
test "${MAESTRO_ENV:-}" = 1
```

If this fails, report that you are outside a managed pane and stop. Do not inspect
or control another client's focused workspace from outside a managed pane.

## Discover supported commands

Use the installed binary's help as the authority. Use only commands it advertises.

```sh
maestro --help
maestro agent --help
maestro pane --help
maestro workspace --help
maestro tab --help
maestro worktree --help
maestro terminal --help
maestro notification --help
maestro integration --help
maestro daemon --help
maestro action --help
```

Do not use bare `maestro` for discovery: it starts chat. Do not probe a mutating
command by omitting arguments. `workspace create`, for example, can execute with
defaults. For a named daemon, use `maestro attach --daemon <name>` only when the
user requests attachment.

Read identifiers and state from successful JSON responses. Maestro's RPC envelope
uses `success` and `data`, not a `result` wrapper. Do not predict IDs from examples
or sidebar order. Re-read the returned pane identity after moving a pane.

## Preserve caller context

Workspace, tab and pane IDs identify topology. Terminal IDs identify terminal
streams. Agent commands accept a unique live agent name or the hosting pane ID,
not a terminal ID or an agent-kind label.

Inspect caller context without relying on a client's focus:

```sh
printf '%s\n' "$MAESTRO_WORKSPACE_ID" "$MAESTRO_TAB_ID" "$MAESTRO_PANE_ID"
maestro workspace list
maestro tab list --workspace "$MAESTRO_WORKSPACE_ID"
maestro pane current --current
maestro pane list --workspace "$MAESTRO_WORKSPACE_ID"
maestro agent list
```

Use `--current`, an explicit pane ID or a unique live agent name. An omitted target
can select the user's or another client's focused pane.

Default to a sibling pane in the current tab and working directory. Do not create
a workspace, tab, worktree or different working directory unless requested.
Honor a requested split direction. Otherwise inspect the layout: split a wide
pane right and a narrow or tall pane down. Avoid unusably small panes.

```sh
maestro pane layout --pane "$MAESTRO_PANE_ID"
maestro pane split --current --direction right --cwd "$PWD" --no-focus
```

Read the new pane ID from the response before using it. Keep the user's focus in
the calling pane unless they requested a focus change.

## Coordinate recognized agents

Use pane commands for shells and ordinary processes. Use agent commands when
identity or observed agent state matters. An existing available shell pane must
be at its interactive prompt, with no foreground command or editor.

```sh
maestro agent start reviewer --kind <supported-kind> --pane <new-pane-id>
maestro agent prompt reviewer "Review the changes and report actionable findings." --wait --timeout 120000
maestro agent get reviewer
maestro agent read reviewer --source recent-unwrapped --lines 120
```

Choose a kind advertised by `agent start --help`. Pass native agent arguments only
after `--`. Starting an agent does not create or move layout. Names must match
`[a-z][a-z0-9_-]{0,31}` and be unique among live agents. A name follows the pane's
current occupant and is cleared when that agent exits, is released or replaced.

Startup waits for detection and readiness: 30000ms by default, 300000ms maximum.
A blocked startup returns `agent_not_ready`; inspect it before prompting.

`idle` means ready for input and seen in the focused UI. `done` is idle after
unseen background work finishes. Focusing a tab or targeting a focus command
marks it seen; reads do not. `blocked` means an approval or question UI was
recognized. `unknown` is not proof of completion.

Prompting an already blocked agent rejects input. From a non-working state, an
accepted prompt must first change state within 5000ms or return
`agent_prompt_stalled`. A shorter `--timeout` returns timeout. The wait observes
states, not turns: an already working turn's completion can match.

`--wait` matches idle, done or blocked by default. Repeat `--until` only when a
specific state is needed. Without `--timeout`, settled-state waits are indefinite.

```sh
maestro agent wait reviewer --until blocked --timeout 120000
maestro agent send-keys reviewer esc
maestro agent send-keys reviewer ctrl+c
```

Use logical keys; all keys are validated before writing input. On a failed wait
or `blocked` result, inspect `agent get` and `agent read` before deciding what to
send. Do not answer an approval or question without the user's instruction.

## Run ordinary commands

After creating a sibling pane and reading its ID:

```sh
maestro pane run <new-pane-id> "just test"
maestro pane wait-output <new-pane-id> --match "test result" --timeout 120000
maestro pane read <new-pane-id> --source recent-unwrapped --lines 120
```

`pane run` sends command text and Enter together. `wait-output` searches existing
output immediately, then polls. Choose exactly one of `--match <text>` and
`--regex <pattern>`. Without a timeout, it waits indefinitely.

Use `visible` for the viewport, `recent` for rendered output,
`recent-unwrapped` for logs with soft wraps joined, or `detection` for the plain
bottom-buffer snapshot. Use `--format ansi` only when terminal styling is evidence.

`--lines` can read only available screen and host scrollback. Alternate-screen
rows do not enter host scrollback. If a larger read cannot recover a completed
response, ask the agent to write it as Markdown in a temporary directory and
return its path; read that file. Use this as a fallback, not an initial prompt rule.

## Safety

- Use `--no-focus` for background work unless the user requests a focus change.
- Never close workspaces, tabs or panes you did not create without explicit permission.
- Never stop a daemon from an active workspace unless the user intends to stop its pane processes.
- Never kill the main process. Use a named disposable daemon for isolated experiments.
- Treat exit 2 as a usage error. Inspect operation errors before retrying; do not replay mutations blindly.

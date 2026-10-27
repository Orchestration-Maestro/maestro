# Extension Examples

Example extensions for Maestro. The
[examples tracking issue](https://github.com/Orchestration-Maestro/maestro/issues/2074)
lists each one with its owner.

## Usage

An extension crate is a `cdylib` whose root names its factory. The
[extension guide](../../docs/extensions.md) covers project setup, building,
packaging and loading. The [guest API](../../docs/extensions/guest.md) describes
the interface the factory registers.

Maestro discovers extensions in two directories, in this order:

1. `<cwd>/.maestro/extensions`
2. `<agent_dir>/extensions`

[Extension loading](../../docs/extensions/loading.md) describes the remaining
steps.

## Examples

Keep the table and the run commands below in file-name order, so a new example
inserts at its own place instead of appending at the end of each list.

| File | Registers |
| --- | --- |
| `bookmark.rs` | A `/bookmark` command that labels the last assistant message, so the tree view can find it again. |
| `commands.rs` | A `/commands [extension\|prompt\|skill]` selector for the host's slash commands, with argument completion and optional source-path display. |
| `hello.rs` | A `hello` tool that greets the supplied name. |
| `hidden_thinking_label.rs` | A `thinking-label` command that sets the label shown when thinking blocks are hidden, or resets it. |
| `permission_gate.rs` | A `tool_call` hook that asks before illustrative dangerous bash commands and refuses them without UI. |
| `pirate.rs` | A `/pirate` command that toggles a pirate response style in the supplied prompt. |
| `prompt_customizer.rs` | Tool and skill guidance added to the supplied system prompt. |
| `protected_paths.rs` | A `tool_call` hook that blocks writes and edits whose path contains one of three illustrative substrings, and warns when the host has UI. |
| `reload_runtime.rs` | A `reload-runtime` command that reloads extensions, skills, prompts and themes, and a `reload_runtime` tool that queues that command as a follow-up message. |
| `send_user_message.rs` | `/ask`, `/steer`, `/followup` and `/askwith` commands that send ordinary, steering, follow-up and two-block user messages. |
| `session_name.rs` | A `/session-name` command that sets the session's friendly name, or shows the name now in effect. |
| `shutdown_command.rs` | A `/quit` command and two tools that request a clean shutdown. |
| `system_prompt_header.rs` | A `System: N chars` status holding the current system prompt's UTF-16 length, removed on shutdown. |
| `tools.rs` | A `/tools` settings view that enables or disables tools immediately and restores the selection from the current session branch. |
| `trigger_compact.rs` | A `turn_end` hook that requests compaction above 100,000 tokens and a `trigger-compact` command that requests compaction immediately. |
| [`with-deps/`](with-deps/) | A `parse_duration` tool that parses a human-readable duration to milliseconds with a regular-expression dependency. |
| `working_message_test.rs` | A custom working text and a one-frame indicator requested on every session start. |

### Building an example

Authors need the pinned toolchain with the `wasm32-wasip2` target. Each example
builds to a component library; there is no command-line entry point:

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo build --locked \
  -p maestro-extensions-wasm --example <name> --target wasm32-wasip2 --release
```

### Running an example

The controlled runs call the registered commands, tools and event handlers.
For prompt guidance, `prompt_guidance_uses_supplied_tools_and_skills` loads the
example's source in the in-process controlled host;
`prompt_guidance_component_matches_supplied_options` builds and instantiates the
component. Both invoke its registered `before_agent_start` event handler:

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- bookmark_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- commands_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- hello_component_greets_through_the_host
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- hidden_thinking_label_component_runs_registered_callbacks
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- permission_gate_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- pirate_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- prompt_guidance
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- protected_paths_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- reload_runtime_component_uses_registered_callbacks
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- session_name_component_runs_registered_command
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- shutdown_component_runs_every_registration
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- system_prompt_status
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- tools_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro --test integration -- tools_component_changes_native_session_and_restores_branch
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- trigger_compact_component_matches_host_requests
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- user_delivery_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib with_deps_component_uses_its_parsing_dependency
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- working_message_
```

No provider, paid service or extension installation is required. Loading a
built component into an application is opt-in and needs that application's
component runtime.

### What the command listing example does

`/commands` lists the slash commands the host supplies, grouped as Extensions,
Prompts and Skills in that order. `/commands extension`, `/commands prompt` and
`/commands skill` filter by that exact source; argument completion offers those
three singular labels. An empty inventory or unmatched filter reports that no
commands were found. Dismissing the selector or choosing a group heading does
nothing else.

Choosing a command with a nonempty source path opens a confirmation showing that
path. Confirming also sends it as an informational notification; declining sends
no notification. The extension reads only the host's command metadata and never
opens a source file. It registers no built-in command and discovers no resources.

Build it with the command above using `--example commands`. The controlled
`commands_` runs need no provider, credentials or installation. Loading the built
component is opt-in and needs a component-enabled host; browsing commands also
needs that host's selector and confirmation UI.

### What the shutdown example does

`/quit`, `finish_and_exit` and `deploy_and_exit` ask the host to shut down
through their context, and each one keeps the answer it returns. The deployment
tool is illustrative: it reports progress, returns the environment it was given
and starts no deployment process. Loading the component into an interactive host
is opt-in and requests that host's shutdown.

### What the prompt guidance example does

`before_agent_start` returns a replacement `systemPrompt` built from the supplied
options: the prompt the host assembled, guidance for each active built-in tool in
read, bash, edit and write order, the supplied skill names in their original
order, and this extension's own context section. It discovers no resources and
builds no prompt.

The supplied prompt already holds the user append and any edit an earlier handler
made, so the example never appends that option again. Loading the component into
an application is opt-in.
### What the permission gate does

`tool_call` leaves non-bash tools and commands outside its three patterns alone.
For case-insensitive matches of `rm` followed by `-r`, `-rf` or `--recursive`,
`sudo` as a word, or `chmod`/`chown` followed on the same line by `777`, it asks
`Allow?` with `Yes` and `No` choices. Only `Yes` permits the call; dismissal or
any other answer blocks it with `Blocked by user`. Without UI it blocks with
`Dangerous command blocked (no UI for confirmation)`.

This is an illustrative regex matcher, not a security sandbox or a shell parser.
It can match quoted text and miss dangerous commands; its flag alternatives also
match prefixes such as `-rubbish`. The host still owns execution. Loading the
component is opt-in and needs a component-enabled host; interactive approval
needs that host's UI. The controlled runs deliver calls to both the in-process
adapter and the built component with scripted selections. They execute no shell
commands and need no provider, credentials or installation. Build it with the
command above using `--example permission_gate`.

### What the protected-paths hook does

`tool_call` inspects only the `write` and `edit` tools and reads their `path`
argument without normalizing it. A path containing `.env`, `.git/` or
`node_modules/` is blocked with `Path "<path>" is protected`; when the host has
UI, the hook first warns `Blocked write to protected path: <path>` with warning
severity. Every other tool and path is left alone and returns no verdict.
Matching is a case-sensitive literal substring test: it does not resolve dot
segments, fold case, or look at the filesystem. The three patterns are this
example's own policy, not a built-in restriction; loading the component into an
application is opt-in.

The example needs a component-enabled host to load. The controlled run needs no
provider, credentials or installation: it delivers every recorded tool call to
the built component and to the in-process adapter.

### What the hidden thinking label example does

`/thinking-label <text>` sets the collapsed-thinking label the host shows, and
`/thinking-label` without arguments clears it and restores `Pondering...` for the
next session start. The example also applies the label it retains at every
`session_start`, so the next session shows the label the session before set.

A command that sets a label trims the surrounding whitespace and keeps the interior
text as it is. When the host rejects a request the command ends with the host's
message, and the label the example retains stays the one the command chose.
Loading the component into an application is opt-in and needs a host that enables
the component runtime.

### What the system prompt status example does

On `agent_start` the example reads the prompt the host supplies at that moment
and sets the `system-prompt` status to `System: N chars`. `N` counts UTF-16 code
units, so whitespace, a byte order mark and combining sequences count as the
string stores them; nothing is trimmed, rebuilt or cached. `session_shutdown`
removes the status under the same key, including when no start came before it.
A host rejection is returned to the host unchanged. Loading the component into
an application is opt-in.

### Building with-deps

The [`with-deps/main.rs`](with-deps/main.rs) example parses a duration string
with a regular expression. The dependency is the workspace pin, declared once as
a development dependency of this crate, so it reaches the crate's examples and
tests without entering the guest library; only the pinned toolchain and the
`wasm32-wasip2` target are needed:

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo build --locked \
  -p maestro-extensions-wasm --example with_deps --target wasm32-wasip2 --release
```

Its controlled run, listed with the others above, calls the registered tool with
every recorded duration text.

### What the compaction trigger example does

`turn_end` records the context usage the host reports and requests compaction when
a known estimate at or below 100,000 tokens is followed by a known estimate above
it. `/trigger-compact` requests compaction immediately, and its arguments become
the instructions the host passes on, trimmed, or none when they are blank.

Usage the host does not report leaves the last known estimate as it was, and the
command never changes it. Compaction, its notifications and any history change
belong to the host: the extension only asks for them, reports `Compaction started`
before the request and `Compaction completed` or `Compaction failed: <message>`
when the host answers, each one only while a UI is available.

No provider, paid service or extension installation is required for the
controlled run. Loading the built component into an application is opt-in, and a
real compaction also needs that application's model and runtime configuration.

### What the bookmark example does

`/bookmark` labels the newest assistant message and `/unbookmark` removes the
newest label from any entry, so the tree view can show where the important
points of a session are. A blank argument becomes a `bookmark-<epoch
milliseconds>` label. The controlled run replays every recorded case against
the built component with a supplied wall clock, so it needs no provider and no
installation; loading the component into an application is opt-in and needs
that application's component runtime.

### What the runtime reload command does

`/reload-runtime` awaits the reload request its command context supplies and
returns nothing else. The `reload_runtime` tool runs with the ordinary context,
which has no reload method, so it queues `/reload-runtime` as a follow-up user
message and answers that it queued it. The controlled run reloads nothing in a
running application; loading the component is opt-in and needs that
application's component runtime.

### What the pirate example does

`/pirate` starts disabled. Each invocation reports the state it switched to and
ignores its arguments. While the mode is on, `before_agent_start` returns the
supplied prompt followed by this example's own pirate instructions; while it is
off, the handler returns no override and leaves the event as it arrived. The
state belongs to the factory instance, so two loaded copies toggle apart, and a
host that refuses the notification still keeps the switch it made first. Loading
the component into an application is opt-in and needs that application's
component runtime.

### What the working message example does

`session_start` requests the custom working text and a one-frame coloured
indicator through the context it receives, then returns no event result. Every
session-start reason sends the same two requests in that order, and a refusal
the host returns ends the callback with the host's own message.

The controlled run asserts the requests the guest sends. It does not show that
an interactive host keeps the text across agent turns or loader recreation;
that behaviour belongs to the host's own working-loader operations. Installing
the component into an application is opt-in and needs that application's
component runtime.

### What the session naming command does

`/session-name [name]` trims the supplied name and stores it, then announces
`Session named: <name>`. Without a name it reads the name in effect and shows
it as `Session: <current>`, or reports `No session name set`. That name is what
the session selector shows in place of the first message. The command reads and
writes the name through the session operations the host supplies, so it stores
nothing itself; the controlled run names no session in a running application.
Loading the component is opt-in and needs that application's component runtime.

### What the tool selector does

`/tools` opens **Tool Configuration**, with one enabled/disabled setting for each
currently available tool, including extension tools. Each change immediately
updates the active tools and appends a `tools-config` session entry containing
`enabledTools`. Escape closes the view without undoing those changes.

On session start and tree navigation, the example reads only the current branch.
It restores the newest saved selection, discarding tools no longer available;
an empty saved selection disables every tool. Without saved configuration it
keeps the host's active selection. Opening the view refreshes the inventory, so
new tools appear disabled until selected.

The controlled run uses supplied tool inventories, branch entries and terminal
input. The native session run also checks active tools, persisted entries, tree
navigation and reload. Neither run calls a provider or changes project files.
Loading the built component
is opt-in and needs a component-enabled interactive host with custom UI and
settings widgets; loading it permits changes to that session's active tools and
configuration entries.

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
| `dynamic_tools.rs` | A `session_start` handler that registers one echo tool, and a `/add-echo-tool` command that registers more while the session runs. |
| `event_bus.rs` | An `emit` command that sends a notification on the event bus, and a retained listener that reports every notification through the current session's UI. |
| `file_trigger.rs` | An opt-in watcher of `/tmp/agent-trigger.txt` that sends nonempty contents as a custom message and requests a response. |
| `git_checkpoint.rs` | Opt-in Git stash checkpoints keyed by session entry, with confirmation before restoring on a fork. |
| `hello.rs` | A `hello` tool that greets the supplied name. |
| `hidden_thinking_label.rs` | A `thinking-label` command that sets the label shown when thinking blocks are hidden, or resets it. |
| `notify.rs` | An opt-in `agent_end` desktop notification through OSC 777, Kitty OSC 99 or Windows PowerShell. |
| `permission_gate.rs` | A `tool_call` hook that asks before illustrative dangerous bash commands and refuses them without UI. |
| `pirate.rs` | A `/pirate` command that toggles a pirate response style in the supplied prompt. |
| `prompt_customizer.rs` | Tool and skill guidance added to the supplied system prompt. |
| `protected_paths.rs` | A `tool_call` hook that blocks writes and edits whose path contains one of three illustrative substrings, and warns when the host has UI. |
| `qna.rs` | A `/qna` command that extracts questions from the latest complete assistant reply into editable input, with a cancellable loader. |
| `reload_runtime.rs` | A `reload-runtime` command that reloads extensions, skills, prompts and themes, and a `reload_runtime` tool that queues that command as a follow-up message. |
| `send_user_message.rs` | `/ask`, `/steer`, `/followup` and `/askwith` commands that send ordinary, steering, follow-up and two-block user messages. |
| `session_name.rs` | A `/session-name` command that sets the session's friendly name, or shows the name now in effect. |
| `shutdown_command.rs` | A `/quit` command and two tools that request a clean shutdown. |
| `snake.rs` | A `/snake` custom view with arrow/WASD controls, 100 ms ticks and saved games in session entries. |
| `snake/game.rs` | Positions, food sampling and the persisted game state for `snake.rs`. |
| `snake/render.rs` | The bordered Snake board and input hints. |
| `status_line.rs` | Session and turn hooks that show themed ready, in-progress and completed-turn footer status. |
| `system_prompt_header.rs` | A `System: N chars` status holding the current system prompt's UTF-16 length, removed on shutdown. |
| `titlebar_spinner.rs` | A braille title spinner every 80 ms while working; periodic work runs in a host-owned event tail, cancelled on restart, end or shutdown. |
| `tool_override.rs` | An opt-in audited `read` override that records attempts, refuses illustrative sensitive paths and displays the last twenty records with `/read-log`. |
| `tools.rs` | A `/tools` settings view that enables or disables tools immediately and restores the selection from the current session branch. |
| `trigger_compact.rs` | A `turn_end` hook that requests compaction above 100,000 tokens and a `trigger-compact` command that requests compaction immediately. |
| `widget_placement.rs` | Widgets above and below the editor, requested on every session start when the host has a UI. |
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
  -p maestro-extensions-wasm --lib -- dynamic_tools_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- event_bus_component_uses_registered_callbacks
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- file_trigger_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro --test integration -- file_trigger_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- git_checkpoint_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasmtime --test integration -- git_checkpoint_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- hello_component_greets_through_the_host
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- hidden_thinking_label_component_runs_registered_callbacks
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- notify_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- permission_gate_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- pirate_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- prompt_guidance
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- protected_paths_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro --test integration -- qna_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- reload_runtime_component_uses_registered_callbacks
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- session_name_component_runs_registered_command
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- shutdown_component_runs_every_registration
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- snake_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- status_line_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- system_prompt_status
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- titlebar_spinner_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasmtime --test integration -- tool_override_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- tool_override_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- tools_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro --test integration -- tools_component_changes_native_session_and_restores_branch
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- trigger_compact_component_matches_host_requests
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- user_delivery_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- widget_placement
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

### What the file trigger example does

Create `/tmp/agent-trigger.txt` before loading the component into a native host
with filesystem and watch support. On each session start the example watches
that fixed path and, when UI is available, reports `Watching /tmp/agent-trigger.txt`.
To send a prompt from another process, write it to the existing file:

```sh
printf '%s\n' 'Run the tests' > /tmp/agent-trigger.txt
```

A change reads and trims the contents. Nonempty text becomes a displayed
`file-trigger` custom message prefixed with `External trigger: `, with
`triggerTurn: true`; the file is cleared only after the host accepts the message.
Empty or unreadable contents do nothing. A missing file at startup fails watch
creation, so create it first. Each new session replaces the previous watch;
shutdown releases it through the existing watch owner.

Loading this example is opt-in. Anyone able to write the trigger file can ask the
agent to act; use it only in a trusted local environment. A real response uses the
host's configured model and can incur provider charges. The controlled runs need
no credentials or paid service: the component run uses a loopback provider. Tests
create the fixed path exclusively and refuse to touch a file already there. The
nextest file-trigger group runs those tests one at a time; do not run separate
file-trigger test commands concurrently.

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

### What the notification example does

Build with `--example notify` using the command above. Loading the component is
opt-in and needs a native component-enabled host with inherited environment and
stdout. On every `agent_end` it sends `Maestro` / `Ready for input`. A nonempty
`WT_SESSION` selects `powershell.exe` (including from WSL); otherwise a nonempty
`KITTY_WINDOW_ID` selects Kitty's two-part OSC 99 notification. Other environments
use OSC 777, supported by Ghostty, iTerm2, WezTerm and rxvt-unicode. The terminal
must permit desktop notifications. Windows needs `powershell.exe` on PATH and
Windows toast support; the callback waits for that command to finish.

Notification text cannot inject OSC fields or control sequences, and PowerShell
apostrophes are quoted before the Windows DOM creates XML text nodes. The
controlled `notify_` tests capture terminal bytes and supply child-process
results: they send no desktop notifications and need no provider or credentials.
The example registers no command and adds no built-in notification setting.

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

### What the turn status line example does

`session_start` sets `status-demo` to dim `Ready`. Each `turn_start` increments
this factory's counter and shows an accent `●` followed by dim ` Turn N...`.
`turn_end` shows a success `✓` followed by dim ` Turn N complete`. Each hook
reads the current UI theme; the counter stays in place across session starts
and starts at zero in a new factory instance. A host rejection returns its
message without undoing a counted turn.

Build with `<name>` set to `status_line` in the build command above. The
controlled runs invoke the built component with supplied theme and UI answers;
they need no provider, credentials or installation. Loading it into an
application is opt-in and needs that application's component runtime. A UI host
must provide footer status and theme operations to show the status.

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

### What the event bus example does

`/emit [message]` trims its argument and sends it on the fixed `my:notification`
channel with `"from": "/emit command"`, defaulting to `hello` when nothing is
left. The argument is always the message: the command takes no event name, so
`/emit other:event payload` sends that whole text to the same channel.

A retained listener reports each `my:notification` payload through the current
session context's UI as `Event from {from}: {message}` at information severity.
The example reports one notification at session start, keeps the latest session
context when a session is replaced, and reports nothing before the first session
starts.

Any extension can emit on the same channel with an object holding string
`message` and `from` members; other members are ignored. Loading the component
into an application is opt-in and needs that application's component runtime.

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

### What the dynamic tools example does

`session_start` registers one `echo_session` tool, and `/add-echo-tool
<tool_name>` registers further echo tools while the session runs. The argument is
trimmed and lowercased, then admitted only when it holds ASCII letters, digits
and underscores, so `/add-echo-tool Shared_9` registers `shared_9`. Each tool
echoes its message behind `[<name>] ` and returns the name and prefix as details,
and a name that is already registered answers `Tool already registered: <name>`
instead of replacing the first tool.

The names belong to the factory instance, so two loaded copies register apart.
A name stays registered when the host refuses its registration, and a host
refusal ends the command with the host's message. The controlled run replays
every recorded sequence against the built component and needs no provider and no
installation; loading the component into an application is opt-in and needs that
application's component runtime.

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

### What the widget placement example does

On every `session_start`, `widget_placement` reads the host's current UI
availability and does nothing when there is none. With a UI it requests the
`widget-above` widget holding the line `Above editor widget`, then the
`widget-below` widget holding the line `Below editor widget` with the explicit
below-editor placement. The first request passes no options, so the host applies
its own above-editor default.

Each request acquires the UI context again, so a host that changes its binding
after the first request serves the second on the new one. A rejected request
ends the callback with that message; the example retries nothing and removes no
widget the host already accepted. Loading the component into an interactive
application is opt-in and needs that application's component runtime and UI
host; the controlled run only records the ordered requests.

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

### What the Snake game does

Build with `--example snake`, then load the component into a component-enabled
interactive host with custom UI, keyboard helpers, native waits and a WASI random
source. Loading and playing are opt-in; no model, provider or network is needed.
Run `/snake` to open the 40 by 15 board. Arrow keys or WASD steer; each food is
worth 10 points. Escape saves the current game in a `snake-save` session entry
and closes the view. Q closes it and clears the save. A restored game starts
paused: any key except Escape or Q resumes it; those two keys close it without
changing the saved game. After a collision, R or Space restarts with the high
score retained. The newest save is read from the whole session, not only the
current branch. A finished saved game starts a new game with its high score.

The controlled `snake_` runs use the built component with supplied terminal
input, random bytes, history and tick completions. Closing the view cancels its
pending native wait and removes its event listener. The example never writes a
file directly; saving and clearing append ordinary session entries through the
host. Narrow views show fewer columns without changing the game's collision
boundaries.

### What the audited read override does

`tool_override` replaces `read` only when you load it. It records each resolved
path in `<agent_dir>/read-access.log` through the host's file-mutation queue,
including blocked attempts and failed reads. Logging failures do not prevent a
read. `/read-log` shows the last twenty records as an informational notification,
or `No access log found` when the log cannot be read.

The case-sensitive patterns block `.env`, `.env.<suffix>`, and paths containing
`/.ssh/`, `/.aws/` or `/.gnupg/`. The secret/credential filename patterns ignore
case and match singular or plural names ending in `.json`, `.yaml`, `.yml` or
`.toml`. These are literal illustrative patterns, not a security sandbox:
symlinks, alternate spellings and names outside them can bypass the checks.

Allowed reads return text, with optional numeric `offset` and `limit`, and the
total line count. The example decodes invalid UTF-8 with replacement characters
and clips selected text at a whole UTF-8 scalar within 50 KiB, followed by
`[Output truncated at 50KB]`. It supplies no custom renderer. This simplified
reader does not preserve the built-in read tool's image and truncation features.

Build with `--example tool_override` in the command above. Loading is opt-in and
requires a component-enabled native host with filesystem access; recording also
needs an existing writable agent directory. Log display needs the host's
notification UI. Logs contain paths, timestamps and access decisions, not file
contents. The controlled run uses disposable files and no provider or credentials.

### What the titlebar spinner does

`agent_start` restores the base title, then shows the ten braille frames in order
at 80 ms intervals. The first frame waits for that interval. Each frame reads the
current session name and working directory again, producing
`<frame> Maestro - <session> - <directory>`, or omitting the session part when the
name is absent or empty. Restarting cancels the old wait and starts at the first
frame; `agent_end` and `session_shutdown` cancel the wait and restore the base
title. Shutdown also removes the example's event-bus listener.

Build with `--example titlebar_spinner` using the command above. Loading it is
opt-in and needs a component-enabled host with native waits, event-tail execution
and terminal-title UI. The controlled runs supply wait-completion gates and
session names, so they need no provider, paid service or terminal installation.
The example starts no process and reads no files. A title-write or name-read
failure ends the callback or animation with the host's error; tail errors go to
the host's event-bus reporter.

### What the Git checkpoint example does

Build with `--example git_checkpoint` using the command above. Loading it is
opt-in and needs a component-enabled native host, Git on `PATH`, and a Git
repository with an initial commit and a configured author identity. Use a
disposable checkout first: approving a restore runs `git stash apply` against
the host's working directory and can change tracked files or leave conflicts.
Interactive restoration also needs the host's selector UI.

`tool_result` remembers the current session leaf when one exists. `turn_start`
awaits `git stash create` and associates a nonempty object ID with that entry,
replacing its previous checkpoint. Creating the object leaves the working tree
and stash list alone; it does not include untracked files. A clean tree creates
no checkpoint and keeps any previous one.

Before a fork, a matching checkpoint and an available UI open **Restore code
state?** with **Yes, restore code to that point** and **No, keep current code**.
A selection starting with `Yes` awaits `git stash apply` and reports
`Code restored to checkpoint` only if Git exits with code zero without being
terminated. A nonzero exit or terminated command fails the callback with the
exit code, termination flag and Git's standard error instead; inspect any partial
changes or conflicts before continuing. Dismissal, another choice, a missing
checkpoint or a headless host leave the working tree alone.

`agent_end` clears the in-memory checkpoint map, not the remembered entry or
Git's unreachable objects. There is no persistent recovery catalog, automatic
reset, stash push, or built-in checkpoint policy. The controlled runs load the
built component through the production runner in disposable repositories with
scripted UI choices. They need Git but no provider, credentials or paid service.

### What the question extractor does

`/qna` reads the latest complete assistant reply on the current branch. It skips
non-text blocks and joins text blocks with newlines, without trimming them. An
incomplete newest assistant reply is rejected rather than replaced with an older
one. A reply without text blocks is skipped; a reply with one empty text block
reports that no assistant messages were found.

The command replaces the editor with a cancellable bordered loader while the
selected model extracts questions. Escape cancels the view. Missing credentials,
completion failures and aborted responses report `Cancelled` and leave the editor
unchanged. A completed extraction replaces the editor text exactly as returned,
so the user can fill in answers before submitting. Closing the view disposes the
loader and releases its pending extraction.

Build with `--example qna` using the command above. Loading is opt-in and needs a
component-enabled interactive host, a selected model and its configured API key.
A real extraction sends the assistant reply to that provider and can incur
charges. The controlled `qna_` runs use a loopback provider and supplied terminal
input; they require no credentials, paid service or installation.

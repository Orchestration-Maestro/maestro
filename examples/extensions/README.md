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
| `confirm_destructive.rs` | Confirmation before starting a new session, switching away from user history or forking. |
| `custom-provider-anthropic/main.rs` | An opt-in `custom-anthropic` provider with two models, its own message stream, `CUSTOM_ANTHROPIC_API_KEY` configuration and subscription login. |
| `custom-provider-anthropic/oauth.rs` | Subscription login with a pasted authorization code, and token exchange and refresh, for `main.rs`. |
| `custom-provider-anthropic/pkce.rs` | The SHA-256 proof key and challenge for that login. |
| `custom-provider-anthropic/request.rs` | Message, tool, thinking and header conversion for one HTTP request. |
| `custom-provider-anthropic/response.rs` | Streamed text, thinking, tool-call, usage and cost events gathered into one assistant message. |
| `custom-provider-anthropic/sse.rs` | Server-sent event framing over raw response bytes. |
| `custom-provider-anthropic/stream.rs` | One request's HTTP call, status check, cancellation and final result. |
| `custom_footer.rs` | A `/footer` command that toggles a footer with token usage, cost, model and Git branch. |
| `custom_header.rs` | A themed startup illustration and a `/builtin-header` command that restores the built-in header and keybinding hints. The guest interface exposes no version, so the subtitle omits it. |
| `dynamic_tools.rs` | A `session_start` handler that registers one echo tool, and a `/add-echo-tool` command that registers more while the session runs. |
| `event_bus.rs` | An `emit` command that sends a notification on the event bus, and a retained listener that reports every notification through the current session's UI. |
| `file_trigger.rs` | An opt-in watcher of `/tmp/agent-trigger.txt` that sends nonempty contents as a custom message and requests a response. |
| `git_checkpoint.rs` | Opt-in Git stash checkpoints keyed by session entry, with confirmation before restoring on a fork. |
| `handoff.rs` | A `/handoff <goal>` command that generates an editable prompt from the selected branch, then opens a parent-linked replacement session. |
| `hello.rs` | A `hello` tool that greets the supplied name. |
| `hidden_thinking_label.rs` | A `thinking-label` command that sets the label shown when thinking blocks are hidden, or resets it. |
| `input_transform.rs` | An `input` hook that rewrites `?quick` questions and handles `ping` and `time` locally. |
| `model_status.rs` | A `model_select` handler that names the selected model and sets the `model` status. |
| `notify.rs` | An opt-in `agent_end` desktop notification through OSC 777, Kitty OSC 99 or Windows PowerShell. |
| `overlay_test.rs` | An `/overlay-test` overlay with inline text fields, wide characters, themed colors and emoji that reports the chosen entry. |
| `permission_gate.rs` | A `tool_call` hook that asks before illustrative dangerous bash commands and refuses them without UI. |
| `pirate.rs` | A `/pirate` command that toggles a pirate response style in the supplied prompt. |
| `preset.rs` | A `--preset` flag, `/preset` selector and Ctrl+Shift+U shortcut for named model, thinking, tool and prompt configurations. |
| [`preset/config.rs`](preset/config.rs) | Layered configuration loading for `preset.rs`. |
| [`preset/selector.rs`](preset/selector.rs) | Retained, themed preset selector for `preset.rs`. |
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
| `todo.rs` | A `todo` tool that stores snapshots in result details, restores the active branch and opens an interactive list with `/todos`. |
| `todo/view.rs` | The todo example's themed tool rows and interactive list view. |
| `tool_override.rs` | An opt-in audited `read` override that records attempts, refuses illustrative sensitive paths and displays the last twenty records with `/read-log`. |
| `tools.rs` | A `/tools` settings view that enables or disables tools immediately and restores the selection from the current session branch. |
| `trigger_compact.rs` | A `turn_end` hook that requests compaction above 100,000 tokens and a `trigger-compact` command that requests compaction immediately. |
| `truncated_tool.rs` | An opt-in ripgrep search tool that applies the host's line/byte limits, saves complete truncated output to a temporary file, and renders calls and results. |
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
  -p maestro-extensions-wasm --lib -- confirm_destructive_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- custom_footer_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- custom_header_component_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- custom_message_provider_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro --test integration -- custom_message_provider_
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
  -p maestro-extensions-wasmtime --test integration -- handoff_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- hello_component_greets_through_the_host
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- hidden_thinking_label_component_runs_registered_callbacks
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- input_transform_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- model_status_component_runs_selection_hook
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- model_status_selection_preserves_literal_identifiers
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- notify_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasmtime --test integration -- overlay_test_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- permission_gate_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- pirate_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- preset_
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
  -p maestro-extensions-wasm --lib -- todo_
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
  -p maestro-extensions-wasm --lib -- truncated_tool_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasmtime --test integration -- truncated_tool_component_
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

### What the custom header example does

On `session_start`, a host with UI receives a custom header factory. Its retained
view draws a block-character North Star over an M, with a `Custom header`
subtitle. It uses the live theme supplied to the factory, ignores the render
width and needs no image support. Headless startup makes no UI request.

`/builtin-header` restores the built-in header and its keybinding hints, then
reports `Built-in header restored`. The command forwards both requests even in a
headless host, where the host decides how to handle them. A later session start
installs the custom header again.

The controlled run builds and instantiates the component, supplies the session
and theme observations, renders its retained view and invokes its reset command.
It needs no provider, credentials or installation. Loading the component into an
application is opt-in and needs that application's component runtime; displaying
the header also needs an interactive UI host.

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

### What the truncated search tool does

Build with `--example truncated_tool`, then load the component into a Unix
component-enabled host with `rg` on its `PATH`. The opt-in `rg` tool searches the
supplied directory (or the session's current directory), with an optional file
glob. Patterns, paths and globs are literal process arguments, not shell text.

Results retain the first 2,000 lines or 50 KB, whichever limit is reached first.
When truncated, the tool awaits a queued write of the complete output to
`maestro-rg-*/output.txt` in the host's temporary directory before returning its
path and omitted line/byte counts. Expanded results show the first 20 lines and
the full-output path. No matches and search errors have separate outcomes.

Search output can contain private file contents. Temporary artifacts remain on
disk so the read tool can open them; remove their directories when no longer
needed. Loading and searching are opt-in; no provider or paid service is needed
for the controlled runs above. Those runs use disposable files and a controlled
search process through the production component runtime. They need
`/usr/bin/python3`, not ripgrep.

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

### What the custom footer does

`/footer` switches between the host's default footer and one dimmed line showing
input tokens, output tokens, total cost, the current model and the Git branch.
Each render totals the assistant messages on the active session branch; it ignores
other messages and entries. Counts below 1,000 are shown as numbers, larger counts
as one decimal followed by `k`, and cost as dollars with three decimal places.
Without a model the footer shows `no-model`; without a Git branch it omits the
parenthesized branch. The host's terminal helpers measure and clip the line to
its supplied width, preserving styling and whole graphemes.

Branch changes request a render. Restoring the default footer disposes the custom
one and unsubscribes its branch listener. The controlled run invokes the built
component with supplied history, model, branch, theme and terminal-helper replies;
it needs no provider, repository, credentials or installation. Loading the
component is opt-in and needs an application with the component runtime and an
interactive UI host. Build it with the command above using `custom_footer` as
the example name.

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

### What the model status example does

`model_select` reads the selection the host reports. On an explicit selection or
a cycle it notifies `Model: {provider}/{id}` with info severity; restoring a
saved selection skips the notification. Every selection then sets the `model`
status to `🤖 {id}` and writes the previous and selected identifiers and
the selection source to standard output. An absent previous model reads `none`.
Repeated and unchanged selections are reported too.

The example makes no model or provider call and paints no footer; a host that
shows the status decides where it goes. `model_status_component_runs_selection_hook`
instantiates the built component and checks its host requests and its standard
output against the recorded selections. Loading the component into an application
is opt-in and needs that application's component runtime.

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

### What the preset example does

Build with `--example preset` in the build command above. The component reads
`presets.json` in `MAESTRO_CODING_AGENT_DIR` (default `~/.maestro/agent`) and
`<cwd>/.maestro/presets.json` at session start. A project entry replaces the whole
global entry with the same name. Missing files are ignored; unreadable files,
invalid JSON or invalid field types report a diagnostic and leave the other
layer available.

Each file maps names to optional `provider`, `model`, `thinkingLevel`, `tools`
and `instructions` fields. For example:

```json
{
  "plan": {
    "thinkingLevel": "high",
    "tools": ["read", "grep", "find", "ls"],
    "instructions": "Understand the problem before proposing changes."
  }
}
```

`--preset plan` applies the preset before session startup completes, including
model selection. `/preset plan` selects it directly; `/preset` opens **Select
Preset**. Ctrl+Shift+U cycles names in sorted order, including `(none)` to clear.
Clearing restores the model, thinking level and tools captured before the first
application. Without a snapshot it restores `read`, `bash`, `edit` and `write`.
Unknown models, missing authentication and unknown tools produce warnings;
nonempty valid tool selections still apply. Empty or wholly invalid tool lists
leave the host's tools unchanged.

Active instructions append to the supplied system prompt. The footer shows
`preset:<name>`. Turn-start entries retain the active name, including a cleared
choice, on the current branch. Session start and tree navigation restore only
the newest name on that branch, without reapplying model or tool settings.
The flag takes precedence over the saved name.

Loading is opt-in and needs a component-enabled native host with filesystem and
environment access; browsing the selector also needs custom UI and retained
widgets. Adding `provider` and `model` selects that exact configured registry
entry and needs its authentication for real requests. This example changes
session settings and appends session entries, but writes no configuration file.
Real prompts can incur provider charges. The `preset_` controlled runs use
read-only fixture files, supplied registry replies and terminal input; they
need no credentials, installation or paid services.

### What the input transform example does

`?quick <question>` returns `Respond briefly in 1-2 sentences: <question>`,
trimming the question's edges with the host's shared whitespace definition.
An empty question warns `Usage: ?quick <question>` and stops. `ping` and `time`
ignore ASCII case and stop after informational feedback: `pong` or the host's
current local date and time. Other text, including extension-injected input,
continues unchanged. The rewrite omits replacement images, leaving the host to
keep the supplied attachments.

Time uses the host's local time zone with the en-US pattern
`M/D/YYYY, h:mm:ss AM`; other locale patterns are not selected. Build with
`--example input_transform` in the command above. The controlled `input_transform_`
runs supply whitespace and clock replies, invoke both adapters and need no
provider, credentials or installation. Loading is opt-in and needs a
component-enabled host with notification UI. Quick questions still use that
host's configured model and can incur provider charges; ping, time and empty
quick input do not call a model.

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

### What the overlay demo does

`/overlay-test` opens a floating overlay 70 cells wide over the conversation.
Its upper rows hold wide CJK and Hangul text, words in five theme colors, and
emoji: a joined sequence, a flag and single symbols. The right border lines up
on every row when the host measures cells and composes the overlay over the text
beneath it correctly. The menu below has four entries. Find and Launch hold an
inline text field; Preferences and Dismiss do not.

The up and down arrows move the selection and stop at either end. On an entry
with a field, typed characters go in at the cursor, Backspace deletes the
character before it, and the left and right arrows move it within the text.
Other control input is ignored. A field too short for its text scrolls sideways
to keep the cursor in view, centered until the cursor comes close to either end
of the text; Enter still reports the whole text. The cell under the cursor is
drawn in reverse video. While the overlay has focus, it also places the terminal's hardware
cursor on that cell, so an input method can show its composition there. Enter
closes the overlay and reports the entry, for example `Find: "query"`, or only
its name when its field is empty or it has none. Escape closes it without a
report.

Build with `--example overlay_test` in the command above. Loading is opt-in and
needs a component-enabled interactive host with custom UI overlays and
notifications. The controlled `overlay_test_` runs load the built component
through the production runner and show the overlay on the real frame writer over
a recording terminal with supplied key input. They need no provider,
credentials or network.

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

### What the branch-aware todo example does

The `todo` tool accepts `list`, `add` with nonempty `text`, `toggle` with an `id`,
and `clear`. Text is kept verbatim, including whitespace. Every result carries
all items and the next identifier in its details, even when an action reports an
error. Clearing removes all items and restarts identifiers at one. These are
session-history snapshots, not files and not the default todo package.

On `session_start` and `session_tree`, the example replays only `todo` tool
results on the host's current branch. The newest snapshot wins; a branch without
one starts empty. Tool rows show themed calls and results, with five items in a
collapsed list and all items when expanded.

`/todos` shows all items and their completion count. Escape or Ctrl+C closes the
view. Without UI it reports `/todos requires interactive mode`. Build with
`--example todo`; `todo/view.rs` is its support module, not a separate target.
The controlled `todo_` runs invoke the registered callbacks with supplied branch,
terminal, helper and theme inputs. They need no provider, credentials or files.
Loading the component is opt-in and needs a component-enabled host; the list view
also needs that host's custom UI, key and terminal-width helpers. Using the tool
through a real model can incur provider charges.

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

### What the handoff example does

`/handoff <goal>` selects the current branch's conversation. After compaction it
uses the latest summary, the kept prefix and subsequent messages. Message and
compaction entries are included; settings, labels and other entry kinds are not.
It asks the selected model for a focused prompt, shows a cancellable loader, and
opens **Edit handoff prompt** for review.
Only response text enters the draft, with a newline between text blocks.

Accepting the editor opens a new session with the current session file as its
parent. The replacement context puts the edited prompt in that session's editor
and reports `Handoff ready. Submit when ready.` It does not submit the prompt.
Cancelling generation or the editor leaves the session alone; a host veto of
replacement reports `New session cancelled` without writing to another editor.
Missing credentials or generation failures report `Cancelled`; the generation
error is also written to the host's error output.

Build with `--example handoff` using the command above. Loading it is opt-in and
requires a component-enabled interactive host with custom UI, editor dialogs,
event tails and session replacement. Real generation sends the selected history
and goal to the configured model and requires that provider's API key; it can
incur charges. The controlled `handoff_` run uses an in-memory provider, history,
credentials and terminal, with no network calls or extension installation.

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

### What the session confirmation example does

Build with `--example confirm_destructive` using the command above. Loading is
opt-in and needs a component-enabled host. Interactive confirmation needs that
host's dialog and notification UI; headless hosts continue without a veto.

Starting a new session always asks **Clear session?**, even when history is empty.
Resuming another session asks **Switch session?** only when the current session
contains a user message anywhere in its entries. Other entry kinds do not count.
Declining or dismissing either confirmation reports cancellation and vetoes the
replacement. Approving returns no override.

Before a fork, the selector shows the first eight characters of the entry ID.
Only **Yes, create fork** permits it; **No, stay in current session**, dismissal
and any other answer report `Fork cancelled` and veto it. The host owns session
replacement and forking: this example only returns cancellation decisions and
never deletes messages or changes files itself.

The controlled `confirm_destructive_` runs replay scripted history and UI answers
through both the in-process adapter and the built component. They need no
provider, credentials, filesystem changes or extension installation. A host
rejection propagates unchanged instead of becoming an approval.

### What the custom message provider does

Build with `--example custom_message_provider` using the command above. Loading
is opt-in and needs a component-enabled host with native HTTP. The example
registers the `custom-anthropic` provider with the `custom-anthropic-api`
protocol at `https://api.anthropic.com`, and two reasoning models that accept
text and images: `claude-opus-4-5` and `claude-sonnet-4-5`. Each has a
200,000-token context window and 64,000 output tokens. Select one with `/model`,
for example `custom-anthropic/claude-sonnet-4-5`.

To use an API key, set `CUSTOM_ANTHROPIC_API_KEY` before starting the host. To
use a subscription, run `/login custom-anthropic`, open the URL it shows, sign
in, then paste the returned `code#state`. A state that does not match the issued
proof key is rejected. Saved tokens count as expired five minutes early and
refresh through the same token endpoint.

Each request is one HTTP attempt with the conversation, tool schemas and the
requested thinking budget. Without a limit, a request asks for a third of the
model's output tokens. The response streams text, thinking and tool calls, then
usage and a cost from the model's prices. A missing key fails before any HTTP
request, and cancelling settles the message as aborted. A non-success status ends
the message with `HTTP <status>` and the response body, so the session's automatic
retry handles rate limits and server errors. The example does not retry on its
own.

Real logins and requests reach the provider and can incur charges. The
controlled `custom_message_provider_` runs use loopback HTTP and scripted login
input; they need no credentials or paid service.

# Examples

The [terminal demonstrations](terminal/examples.md) exercise chat editing, image
rendering, key input and viewport history through the native terminal adapter.
Run them directly with Cargo in a disposable Unix terminal.

## Package examples

The package directory holds an `examples/` folder, beside the README and the
documentation. `examples/README.md` indexes two folders, `extensions/` and
`sdk/`, and each folder has its own index page. Shipped extensions include:
`extensions/confirm_destructive.rs` asks before replacing or forking a session;
its controlled runs supply history and dialog answers without changing a session.
Loading is opt-in and interactive confirmation needs the host's UI.
`extensions/hello.rs` builds a greeting component, `extensions/shutdown_command.rs`
builds a quit command with two tools that ask the host to shut down,
`extensions/prompt_customizer.rs` builds a prompt-guidance component,
`extensions/protected_paths.rs` builds a tool-call hook that blocks writes and
edits to three illustrative path substrings,
`extensions/hidden_thinking_label.rs` builds a command that sets the label shown
for collapsed thinking, and `extensions/with-deps/main.rs` builds one that parses
durations with a regular expression; `extensions/reload_runtime.rs` builds a
reload command with a tool that queues it as a follow-up message,
`extensions/pirate.rs` builds a command that toggles a pirate response style in
the supplied prompt, `extensions/trigger_compact.rs` builds a hook that requests
compaction above a token threshold and a command that requests compaction
immediately, `extensions/bookmark.rs` labels the newest assistant message so the
tree view can show it, `extensions/working_message_test.rs` asks for custom
working text and an indicator whenever a session starts,
`extensions/dynamic_tools.rs` registers one echo tool when a session starts and
a command that adds more echo tools while the session runs,
`extensions/session_name.rs` builds a command that sets or shows the session's
friendly name, `extensions/system_prompt_header.rs` reports the current system
prompt's length as status, `extensions/send_user_message.rs` builds commands
that send ordinary, steering, follow-up and two-block user messages, and
`extensions/model_status.rs` sets the `model` status for each selected model
and notifies only on an explicit selection or a cycle, never on a restore.
`extensions/widget_placement.rs` requests widgets above and below the editor on
session start. `extensions/qna.rs` extracts questions from the latest complete
assistant reply into the editor through the configured model, with a cancellable
loader; real extraction is opt-in and requires interactive UI and credentials.
`extensions/git_checkpoint.rs` creates opt-in Git stash checkpoints for session
entries and asks before applying one when forking; its controlled run uses
disposable repositories and scripted UI choices. Loading it requires Git and a
committed repository, and approving a restore can change tracked files or leave
conflicts. `extensions/snake.rs` opens a custom Snake game with keyboard controls,
timed movement and saved games in session entries; its index gives the build/run
commands and interactive-host prerequisites.
`extensions/overlay_test.rs` opens a floating overlay with inline text fields,
wide characters, theme colors and emoji, for checking how the host composes an
overlay and places the cursor; its index entry lists the controlled run.
`extensions/status_line.rs` shows themed readiness and counted turn progress in
the footer; its controlled run is listed in the extension index.
`extensions/notify.rs` sends an opt-in desktop notification on agent completion
through OSC 777, Kitty OSC 99 or Windows PowerShell; its
[index entry](../examples/extensions/README.md#what-the-notification-example-does)
lists the terminal and platform prerequisites and controlled run command.
`extensions/truncated_tool.rs` wraps ripgrep with the host's output limits,
retains complete truncated output in a temporary file, and renders search calls
and results. It needs a Unix component host with `rg` on its `PATH`; loading it
is opt-in. Temporary output can contain private contents and remains until
removed. `extensions/custom-provider-anthropic/main.rs` registers an opt-in
provider with two models, its own message stream, an API-key variable and
subscription login; its index entry lists the build and controlled run commands.
The
[examples tracking issue](https://github.com/Orchestration-Maestro/maestro/issues/2074)
lists the examples still to come with their owners.

`extensions/handoff.rs` generates an editable prompt from the selected branch
and opens a parent-linked session without submitting it. Loading it is opt-in
and needs an interactive component host with custom UI, editor dialogs, event
tails, session replacement and configured model credentials. Real generation
sends the selected history to that model and can incur charges. The extension
index lists the controlled `handoff_` run, which needs no live service.

A test checks both directions: every entry of a folder is listed in that
folder's index, and every relative link or table row an index carries names a
path that exists. The extension index keeps its table and its run commands in
file-name order, so a new example inserts at its own place.

The default system prompt names this folder, and `get_examples_path` resolves it
from the package directory, so a source run and a standalone build expose the
same path. The standalone copy reads the README, the release history, the
documentation and the examples from the package directory.

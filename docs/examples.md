# Examples

The [terminal demonstrations](terminal/examples.md) exercise chat editing, image
rendering, key input and viewport history through the native terminal adapter.
Run them directly with Cargo in a disposable Unix terminal.

## Package examples

The package directory holds an `examples/` folder, beside the README and the
documentation. `examples/README.md` indexes two folders, `extensions/` and
`sdk/`, and each folder has its own index page. Fifteen extensions ship:
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
`extensions/session_name.rs` builds a command that sets or shows the session's
friendly name, `extensions/system_prompt_header.rs` reports the current system
prompt's length as status, and `extensions/send_user_message.rs` builds commands
that send ordinary, steering, follow-up and two-block user messages.
`extensions/status_line.rs` shows themed readiness and counted turn progress in
the footer; its controlled run is listed in the extension index. The
[examples tracking issue](https://github.com/Orchestration-Maestro/maestro/issues/2074)
lists the examples still to come with their owners.

A test checks both directions: every entry of a folder is listed in that
folder's index, and every relative link or table row an index carries names a
path that exists. The extension index keeps its table and its run commands in
file-name order, so a new example inserts at its own place.

The default system prompt names this folder, and `get_examples_path` resolves it
from the package directory, so a source run and a standalone build expose the
same path. The standalone copy reads the README, the release history, the
documentation and the examples from the package directory.

# Extension examples

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

## Adding an example

1. Add the Rust source here, with a `//!` opening paragraph describing it.
2. Add usage and controlled-run commands in a same-name Markdown page.
3. Add `src/tests/<name>_example.rs` in `maestro-extensions-wasm`. The build
   script discovers these files; do not add a `mod` line to `src/tests.rs`.
4. Add a `[[example]]` block to that crate's manifest in alphabetical order,
   with the source path and `crate-type = ["cdylib"]`.
5. Run `just example-index`, then run the example's controlled tests.

The table below is generated from each Rust file's opening documentation and
Markdown page's heading. Do not edit its rows by hand. `just example-index-check`
checks that it is current. Keep descriptions and run commands in the example's
own files, not in this page or `docs/examples.md`.

Cargo needs an explicit target for each out-of-package component. Those manifest
blocks and the tracked table remain shared edits; alphabetical neighbours can
still conflict. Regenerate the table after merging another example. Examples
with additional native-runner tests also need an explicit alphabetical module
entry in `crates/maestro-extensions-wasmtime/tests/integration.rs`, as required by
the workspace integration-test conventions. Retain both sides’ Cargo target
blocks and native integration module declarations in alphabetical order when
resolving conflicts, then regenerate the table.

## Examples

<!-- example-index:start -->
| File | Provides |
| --- | --- |
| `bookmark.md` | Bookmark |
| `bookmark.rs` | Entry bookmarks: commands that label session entries, so that the tree view can find the important points of a conversation again. |
| `commands.md` | Commands |
| `commands.rs` | Lists the host's slash commands through an opt-in `/commands` selector. |
| `confirm_destructive.md` | Confirm destructive |
| `confirm_destructive.rs` | Opt-in confirmation before replacing or forking a session. |
| `custom_compaction.md` | Custom compaction |
| `custom_compaction.rs` | Summarizes the complete prepared conversation with a separately selected model. |
| `custom_footer.md` | Custom footer |
| `custom_footer.rs` | A command that toggles a footer showing history usage, the model and Git branch. |
| `custom_header.md` | Custom header |
| `custom_header.rs` | Replaces the startup header with a themed illustration and restores it on command. |
| `dynamic_tools.md` | Dynamic tools |
| `dynamic_tools.rs` | Dynamic tools: one echo tool registered when a session starts, and a command that adds more echo tools while the session runs. |
| `event_bus.md` | Event bus |
| `event_bus.rs` | A command that emits events for other extensions, and a listener that reports them. |
| `file_trigger.md` | File trigger |
| `file_trigger.rs` | An opt-in file watcher that sends external triggers to the conversation. |
| `git_checkpoint.md` | Git checkpoint |
| `git_checkpoint.rs` | Opt-in Git stash checkpoints for restoring code when forking a conversation. |
| `handoff.md` | Handoff |
| `handoff.rs` | Generates an editable prompt from the current branch for a new, parent-linked session. |
| `hello.md` | Hello |
| `hello.rs` | A greeting tool registered by an extension factory. |
| `hidden_thinking_label.md` | Hidden thinking label |
| `hidden_thinking_label.rs` | An extension that sets the label shown when thinking blocks are hidden. |
| `input_transform.md` | Input transform |
| `input_transform.rs` | An opt-in input hook: `?quick <question>` requests a brief answer; `ping` and `time` show immediate feedback without a model call. Extension-injected input passes through. |
| `minimal_mode.md` | Minimal mode |
| `minimal_mode.rs` | Opt-in minimal tool rows with full expanded output. |
| `model_status.md` | Model status |
| `model_status.rs` | A model-status extension that reports the model each selection chose. |
| `notify.md` | Notify |
| `notify.rs` | Sends an opt-in desktop notification when the agent is ready for input. |
| `overlay_test.md` | Overlay test |
| `overlay_test.rs` | A floating overlay for checking how the terminal composes one over the conversation. |
| `permission_gate.md` | Permission gate |
| `permission_gate.rs` | An opt-in confirmation hook for illustrative dangerous bash patterns. |
| `pirate.md` | Pirate |
| `pirate.rs` | A command that toggles a pirate response style in the assembled system prompt. |
| `preset.md` | Preset |
| `preset.rs` | Named model, thinking, tool and prompt configurations selected for one session. |
| `preset/config.rs` | Layered preset files read through the host's authored-path helpers. |
| `preset/selector.rs` | Themed preset selection through the host's retained list and container widgets. |
| `prompt_customizer.md` | Prompt customizer |
| `prompt_customizer.rs` | A prompt-guidance extension that reads the supplied prompt options. |
| `protected_paths.md` | Protected paths |
| `protected_paths.rs` | An opt-in tool-call hook that blocks writes and edits to illustrative path substrings. |
| `qna.md` | Qna |
| `qna.rs` | Extracts questions from the last complete assistant reply into the editor. |
| `reload_runtime.md` | Reload runtime |
| `reload_runtime.rs` | A command that reloads runtime resources and a tool that queues that command. |
| `send_user_message.md` | Send user message |
| `send_user_message.rs` | User-message delivery: commands that send ordinary, steering, follow-up and structured messages, so a user can reach the agent from an extension. |
| `session_name.md` | Session name |
| `session_name.rs` | A command that sets or shows the session's friendly name. |
| `shutdown_command.md` | Shutdown command |
| `shutdown_command.rs` | A quit command and two tools that ask the host to shut down. |
| `snake.md` | Snake |
| `snake.rs` | Play Snake in a custom terminal view, saving a paused game in session entries. |
| `snake/game.rs` | Snake positions, food and the state stored in session entries. |
| `snake/render.rs` | The game's bordered, two-column terminal cells and input hints. |
| `space_invaders.md` | Space invaders |
| `space_invaders.rs` | Play Space Invaders in a custom terminal view with session-entry saves. |
| `space_invaders/game.rs` | Retained game records and one deterministic simulation tick. |
| `space_invaders/render.rs` | Original terminal sprites, colours, borders and status text. |
| `status_line.md` | Status line |
| `status_line.rs` | Shows themed turn progress as persistent footer status. |
| `summarize.md` | Summarize |
| `summarize.rs` | An opt-in conversation summary shown in a custom Markdown view. |
| `system_prompt_header.md` | System prompt header |
| `system_prompt_header.rs` | Reports the effective system prompt's length as extension status. |
| `titlebar_spinner.md` | Titlebar spinner |
| `titlebar_spinner.rs` | Animates the terminal title while the agent works, restoring it when work stops. |
| `todo.md` | Todo |
| `todo.rs` | A todo tool whose result snapshots follow the active session branch. |
| `todo/view.rs` | Tool rows and the interactive todo list, using the host's theme and terminal helpers. |
| `tool_override.md` | Tool override |
| `tool_override.rs` | An opt-in read override with access records and illustrative sensitive-path checks. |
| `tools.md` | Tools |
| `tools.rs` | Interactive tool selection retained in the current session branch. |
| `trigger_compact.md` | Trigger compact |
| `trigger_compact.rs` | A turn hook that requests compaction above a usage threshold and a command that requests it immediately. |
| `truncated_tool.md` | Truncated tool |
| `truncated_tool.rs` | A ripgrep tool that retains full output when the host's output limits are reached. |
| `widget_placement.md` | Widget placement |
| `widget_placement.rs` | A startup extension that places text widgets above and below the editor. |
| `with-deps.md` | With deps |
| `with-deps/main.rs` | A duration tool registered by an extension that declares its own parsing dependency. |
| `working_message_test.md` | Working message test |
| `working_message_test.rs` | A startup extension that requests a custom working text and indicator. |
<!-- example-index:end -->

### Building an example

Authors need the pinned toolchain with the `wasm32-wasip2` target. Each example
builds to a component library; there is no command-line entry point:

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo build --locked \
  -p maestro-extensions-wasm --example <name> --target wasm32-wasip2 --release
```

The corresponding Markdown page lists its controlled runs and host prerequisites.
No provider, paid service or extension installation is required for those runs.
Loading a built component is opt-in and needs the application's component runtime.

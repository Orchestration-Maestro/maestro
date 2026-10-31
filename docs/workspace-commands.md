# Workspace command discovery

The command-line library provides command metadata, help, shell completions and
annotated defaults without starting a daemon, a model or package code. Native
executable dispatch is not wired to these presenters yet. The grammar below is
the command contract, not a claim that every operation is executable today.

Bare `maestro` keeps its chat meaning. `--session <path|id>` selects a chat session;
`--skill <path>` loads a skill. Workspace operations use these first words:

| Noun | Purpose |
| --- | --- |
| `attach` | Attach to a workspace daemon; select one with `--daemon <name>`. |
| `daemon` | Run or control a daemon, including named-daemon lifecycle. |
| `status` | Report client/daemon versions, protocol, capabilities and restart requirements. |
| `api` | Inspect the API schema or live snapshot. |
| `completion` | Generate a shell completion script. |
| `settings` | Check preferences, read defaults or reset multiplexer keybindings. |
| `workspace`, `worktree`, `tab`, `pane` | Manage workspace topology and terminal panes. |
| `agent`, `terminal` | Inspect or control recognized agents and terminal streams. |
| `notification`, `integration`, `action` | Present notifications, agent integrations and workspace actions. |

Existing `install`, `remove`/`uninstall`, `update`, `list` and `config` package
commands remain separate from the workspace nouns. There is one package manager.

## Help

`maestro_cli::workspace_commands::spec::command()` returns the shared metadata for
chat flags, package commands and workspace nouns. A drift test checks it against
the chat parser's flag tables; metadata does not replace that parser. Multi-letter
short spellings such as `-nt` stay chat inputs and appear in option descriptions;
completion generators advertise canonical long forms and single-letter shorts.

`spec::write_requested_help` consumes arguments after the executable name. It
prints an exact command path ending in `--help` or `-h`, including root help. It
leaves unrelated tokens and operands after `--` untouched. Root and command-group
help print the caller-supplied shipped skill path; leaf help retains its focused
instructions. Write errors are returned to the executable.

Examples of help paths are `maestro pane --help`, `maestro agent start --help` and
`maestro daemon delete --help`.

## Completions

`maestro_cli::completion::run_completion` consumes the arguments after `completion`
and writes a script for exactly one of `bash`, `elvish`, `fish`, `powershell` or
`zsh`. It renders the same metadata as help. Generation is buffered before the
caller writes output, so a failed output stream returns its original error.
Zsh long options use space-separated values, matching the command grammar.

Once executable dispatch is connected, shell setup uses:

```sh
maestro completion bash > maestro.bash
maestro completion elvish > maestro.elv
maestro completion fish > maestro.fish
maestro completion powershell > maestro.ps1
maestro completion zsh > _maestro
```

Source the generated script through that shell's normal completion setup. No
completion command installs files or starts a runtime.

## Defaults and shipped skill

`maestro settings defaults` has no operands. Its presenter prints the existing
workspace owner's annotated JSONC guide unchanged. Saved `settings.json` remains
strict JSON. See [workspace defaults](settings.md#workspace-defaults-output).

`maestro_app::configuration::config::get_workspace_skill_path()` resolves
`docs/skills/workspace/SKILL.md` beneath the same package directory as the docs
helper. Its `paths` counterpart takes a supplied package directory. The existing
documentation asset copier ships that file in standalone layouts; neither helper
reads the skill or starts package code.

The [workspace skill](skills/workspace/SKILL.md) requires explicit user intent and
a managed-pane environment before control. It uses installed help as authority,
explicit caller targets and no-focus background operations. It does not authorize
answering an approval, closing another client's panes or stopping a daemon.

## Status and output rules

The presenters return 0 for successful discovery and 2 for invalid usage.
Completion help is written to the supplied diagnostic stream; scripts and defaults
go to ordinary output. Application-operation failures use 1. Output failures,
including `BrokenPipe`, remain I/O errors for the executable to settle. The
executable contract maps a closed stdout to silent status 141; that native entry
integration is separate from these effect-free presenters.

Daemon lifecycle grammar and settings reset guarantees are recorded in the
[workspace specification](specs/workspace-daemon.md#command-line-grammar).

# Use aliases in shell commands

Put aliases for Maestro in a dedicated Bash file, then load that file with
`shellCommandPrefix`. This applies to direct session shell commands and the
built-in `bash` tool. It does not change your terminal's shell configuration.

Maestro's native shell runner starts the selected shell with `-c`, without an
interactive flag. Non-interactive Bash does not normally read `~/.bashrc` or
expand aliases. Bash can also read an inherited `BASH_ENV` file; check that
variable if startup runs code you did not expect.

## Configure Bash

1. Create `~/.maestro-shell-aliases.bash` in your editor. Add this harmless alias:

   ```bash
   alias workspace_note='printf "alias ready: %s\n"'
   ```

   Keep this file in Bash syntax even if your terminal uses another shell. Add
   only definitions you want Maestro to use. Sourcing a file executes its code:
   do not source your whole interactive configuration or extract lines from it
   with `grep` and `eval`.

2. Merge these keys into your global `~/.maestro/agent/settings.json`, preserving
   other settings and any existing prefix work you need:

   ```json
   {
     "shellPath": "/bin/bash",
     "shellCommandPrefix": "shopt -s expand_aliases\nsource \"$HOME/.maestro-shell-aliases.bash\" || exit"
   }
   ```

   Use the absolute path printed by `command -v bash` if Bash is elsewhere.
   `shopt` is Bash-specific; selecting `sh` will not work. The global directory
   can differ when you set
   [`MAESTRO_CODING_AGENT_DIR`](application/configuration.md#identity-and-roots).
   [Project settings](sessions/settings.md#scopes-and-merging) override global
   values, so check both scopes if the prefix does not take effect.

   The `\n` is a JSON newline escape. Maestro appends another newline before
   the command. Bash reads the initialization before parsing the command that
   uses the alias. Keep these line breaks: defining an alias and using it on
   the same parsed line can leave it unexpanded. The escaped double quotes keep
   the alias-file path together when `$HOME` contains spaces. `|| exit` stops
   execution if the file cannot be sourced rather than running an uninitialized
   command.

3. Restart Maestro so its built-in tools use the new settings.

## Check both command paths

In the interactive editor, submit:

```text
!workspace_note 'two words'
```

The shell output should be:

```text
alias ready: two words
```

The `!` prefix runs a [direct shell command](chat/commands.md#shell-input)
without asking a model. To check the tool path, ask your configured model to
call the `bash` tool with this exact command:

```text
workspace_note 'two words'
```

Check the tool result for the same output. A model request uses your selected
provider and may cost money; the direct shell check does not need one.

Each command starts a fresh shell. Definitions made during one command do not
carry into the next; the prefix loads the alias file again. Editing this file
changes what subsequent commands source. Remove `shellCommandPrefix` to stop
loading it, or edit the prefix if it also contains initialization you still need.

## If the alias is not found

Check the alias name, the file path, the selected Bash executable and any
project override. A definition in your terminal alone is not enough. Keep the
alias invocation on a line after the initialization. If it is inside a compound
command, load its definition before Bash parses the entire compound command.

SDK callers using raw operations must supply initialization themselves:
[`execute_bash_with_operations`](tools-process.md#direct-shell-execution) runs
literal command text and does not read settings. The application's
`AgentSession::execute_bash` reads the session prefix; standalone tool factories
accept it as `BashToolOptions::command_prefix`. See the
[agent Bash tool](tools-process.md#agent-bash-tool) for operation overrides and
spawn hooks that can replace command preparation.

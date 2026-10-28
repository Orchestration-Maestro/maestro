# Maestro on Windows

Use a Linux build inside WSL for interactive work. Native Windows interactive
startup and selectors are not available yet; [the runtime work is tracked
here](https://github.com/Orchestration-Maestro/maestro/issues/2844). Installing a
Bash shell or changing terminals does not remove that restriction.

## Choose a launch route

| Route | What is verified |
| --- | --- |
| Linux executable inside WSL | Help and one-shot text/JSON output have been exercised with disposable configuration and a local controlled provider. Follow the Unix [first-session guide](first-session.md) inside the distribution for setup and sign-in. |
| Native Windows executable | Compilation and native test runs are recorded under [Windows qualification](https://github.com/Orchestration-Maestro/maestro/issues/486). These do not establish interactive application startup. |

The native qualification records include a run with 12,872 passing tests and
173 passing doctests. Required CI currently runs tests on Linux and compilation
checks for Windows and macOS; see [contributing](../CONTRIBUTING.md#before-submitting-a-pr).
A passing terminal-adapter test is not proof that the executable can open chat.
Native interactive mode, session/configuration selectors and extension selectors
remain with [the Windows startup owner](https://github.com/Orchestration-Maestro/maestro/issues/2844).

Inside WSL, build and run the Linux executable, not `maestro.exe`. After building
with the [first-session source instructions](first-session.md#1-build-from-source), stay in the
checkout and define the same shortcut used in the first-session guide:

```sh
export MAESTRO_SOURCE="$PWD"
maestro() { "$MAESTRO_SOURCE/target/release/maestro" "$@"; }
maestro --help
```

Keep the checkout available for its resources. After configuring credentials and
selecting a model as described in the first-session guide, try a disposable run:

```sh
mkdir -p ../maestro-windows-practice
cd ../maestro-windows-practice
maestro --print --no-session --no-extensions --no-tools "Reply with a short greeting."
maestro --mode json --no-session --no-extensions --no-tools "Reply with a short greeting."
```

Text mode prints the final answer. JSON mode emits newline-delimited session
events. Both send the prompt to the configured provider; they are not offline
checks. `--no-session` avoids saving this run's history. `--no-tools` disables
built-in tools; `--no-extensions` disables discovered extensions. These flags
are not a sandbox. See [print mode](print.md) for output and exit statuses.

## Select the shell for native Windows

The Bash tool and direct shell execution use the following selection order:

1. A nonempty `shellPath` setting. A missing path is an error, with no fallback.
2. `Git\bin\bash.exe` under `ProgramFiles`, then `ProgramFiles(x86)`.
3. The first result from `where bash.exe`, if that path exists.

[Git for Windows](https://git-scm.com/download/win) supplies Git Bash. With the
usual `ProgramFiles` value, Maestro checks
`C:\Program Files\Git\bin\bash.exe`. An installation elsewhere can be selected
with `shellPath` or made discoverable through the launching process's `PATH`.
Known Git Bash locations take precedence over `PATH`; `SHELL` does not select
the executable. Maestro passes `-c` and the command to the selected shell, so
PowerShell and Command Prompt are not Bash replacements.

To select another Bash installation, merge this property into your existing
`settings.json`, preserving other preferences:

```json
{
  "shellPath": "C:\\Tools\\MSYS2\\usr\\bin\\bash.exe"
}
```

Each backslash is doubled because this is JSON. Use the actual executable path,
without embedded quote characters or command arguments. The default user file
is `<home>/.maestro/agent/settings.json`; project settings live in
`.maestro/settings.json` and override user settings. A nonempty
`MAESTRO_CODING_AGENT_DIR` changes the user configuration root. See
[configuration paths](application/configuration.md#identity-and-roots) and
[settings](settings.md). Restart after editing the file.

WSL is a separate Linux environment. Without a custom `shellPath`, the Linux
executable checks `/bin/bash`, then `which bash`, then falls back to `sh`. Use
Linux paths and Linux settings there. Finding a Windows `bash.exe` does not verify a WSL launch route for the
native Windows application.

## Diagnose a missing shell

`Custom shell path not found: ...` means the explicit `shellPath` does not
exist. Correct the path or remove the property to restore discovery. Maestro
checks existence at selection time, not whether a file can actually execute;
a directory or an unusable executable can still fail at spawn time.

`No bash shell found` means neither the known Git Bash locations nor the first
`where bash.exe` result supplied an existing path. The error lists the Git Bash
locations searched. Install Git for Windows, set `shellPath`, or correct the
`PATH` inherited by Maestro. Reopen the launching shell after changing its
environment. A shell fix does not enable the unsupported native selectors.

## Extension file access

Native Windows extension filesystem access is incomplete: the current host
exposes filesystem roots only on Unix. The Windows root mapping is tracked in
[extension filesystem roots](https://github.com/Orchestration-Maestro/maestro/issues/2873).
Do not rely on extension `std::fs` reads or writes on native Windows.

The Windows root-mapping work has a documented reach limit: drives are captured when
an extension loads (including mapped network drives), together with the working
directory's own share. Other UNC shares remain unreachable, and a drive mounted
later requires a reload. [Full Windows path access](https://github.com/Orchestration-Maestro/maestro/issues/2942)
is still open; this guide does not promise that access. These extension limits
are separate from the built-in file tools.

For compiler prerequisites and the Unicode build-directory limitation, use
[Windows source-build troubleshooting](development.md#windows-x86-64-msvc-builds-with-unicode-paths).
A successful build alone does not qualify a runtime mode.

# Notify

Build with `--example notify` using the [build command](README.md#building-an-example). Loading the component is
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

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- notify_
```

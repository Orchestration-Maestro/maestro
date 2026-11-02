# Dynamic tools

`session_start` registers one `echo_session` tool, and `/add-echo-tool
<tool_name>` registers further echo tools while the session runs. The argument is
trimmed and lowercased, then admitted only when it holds ASCII letters, digits
and underscores, so `/add-echo-tool Shared_9` registers `shared_9`. A tool the
command registers echoes its message behind `[<name>] `, `echo_session` behind
`[session] `, and each returns the name and prefix as details. A name that is
already registered answers `Tool already registered: <name>` instead of
replacing the first tool.

The names belong to the factory instance, so two loaded copies register apart.
The factory retains a name even when the host refuses its registration. The
refused request admits no tool, so a retry answers that the name is already
registered, and a host refusal ends the command with the host's message. The
controlled run replays every recorded sequence against the built component and
needs no provider and no installation; loading the component into an application
is opt-in and needs that application's component runtime.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- just prepare-tests
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- dynamic_tools_
```

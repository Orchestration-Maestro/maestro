# Dynamic tools

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

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- dynamic_tools_
```

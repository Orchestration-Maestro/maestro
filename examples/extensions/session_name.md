# Session name

`/session-name [name]` trims the supplied name and stores it, then announces
`Session named: <name>`. Without a name it reads the name in effect and shows
it as `Session: <current>`, or reports `No session name set`. That name is what
the session selector shows in place of the first message. The command reads and
writes the name through the session operations the host supplies, so it stores
nothing itself; the controlled run names no session in a running application.
Loading the component is opt-in and needs that application's component runtime.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- just prepare-tests
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- session_name_component_runs_registered_command
```

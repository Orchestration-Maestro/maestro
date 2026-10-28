# Commands

`/commands` lists the slash commands the host supplies, grouped as Extensions,
Prompts and Skills in that order. `/commands extension`, `/commands prompt` and
`/commands skill` filter by that exact source; argument completion offers those
three singular labels. An empty inventory or unmatched filter reports that no
commands were found. Dismissing the selector or choosing a group heading does
nothing else.

Choosing a command with a nonempty source path opens a confirmation showing that
path. Confirming also sends it as an informational notification; declining sends
no notification. The extension reads only the host's command metadata and never
opens a source file. It registers no built-in command and discovers no resources.

Build it with the [build command](README.md#building-an-example) using `--example commands`. The controlled
`commands_` runs need no provider, credentials or installation. Loading the built
component is opt-in and needs a component-enabled host; browsing commands also
needs that host's selector and confirmation UI.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- commands_
```

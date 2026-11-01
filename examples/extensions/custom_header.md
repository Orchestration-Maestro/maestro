# Custom header

On `session_start`, a host with UI receives a custom header factory. Its retained
view draws a block-character North Star over an M, with a `Custom header`
subtitle followed by the host version in a dimmed `v<version>` suffix. It uses
the live theme supplied to the factory, ignores the render width and needs no
image support. Headless startup makes no UI request.

`/builtin-header` restores the built-in header and its keybinding hints, then
reports `Built-in header restored`. The command forwards both requests even in a
headless host, where the host decides how to handle them. A later session start
installs the custom header again.

The controlled run builds and instantiates the component, supplies the session
and theme observations, renders its retained view and invokes its reset command.
It needs no provider, credentials or installation. Loading the component into an
application is opt-in and needs that application's component runtime; displaying
the header also needs an interactive UI host.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- just prepare-tests
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- custom_header_component_
```

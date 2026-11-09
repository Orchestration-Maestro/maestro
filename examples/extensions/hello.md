# Hello

The `hello` tool greets the supplied name. Its controlled runs load the built
component into the production engine through the
[test harness](../../docs/extensions/testing.md) and call the registered tool.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- just prepare-tests
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasmtime --test integration -- hello_example::
```

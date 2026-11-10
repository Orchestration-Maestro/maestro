# Hello

The `hello` tool greets the supplied name. Its controlled runs load the built
component into the production engine through the
[test harness](../../docs/extensions/testing.md) and call the registered tool.

## Controlled runs

```sh
mise exec -- just prepare-tests
mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasmtime --test integration -- hello_example::
```

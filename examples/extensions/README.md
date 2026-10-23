# Extension Examples

Example extensions for Maestro. The
[examples tracking issue](https://github.com/Orchestration-Maestro/maestro/issues/2074)
lists each one with its owner.

## Usage

An extension crate is a `cdylib` whose root names its factory. The
[extension guide](../../docs/extensions/guest.md) describes the interface the
factory registers.

Maestro discovers extensions in two directories, in this order:

1. `<cwd>/.maestro/extensions`
2. `<agent_dir>/extensions`

[Extension loading](../../docs/extensions/loading.md) describes the remaining
steps.

## Examples

| File | Registers |
| --- | --- |
| `hello.rs` | A `hello` tool that greets the supplied name. |

### Building hello

Authors need the pinned toolchain with the `wasm32-wasip2` target. The example
builds to a component library; there is no command-line entry point:

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo build --locked \
  -p maestro-extensions-wasm --example hello --target wasm32-wasip2 --release
```

### Running hello

The controlled run instantiates the built component in the test host and calls
the registered tool with the recorded names:

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib hello_component_greets_through_the_host
```

No provider, paid service or extension installation is required. Loading the
built component into an application is opt-in and needs that application's
component runtime.

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
| `shutdown_command.rs` | A `/quit` command and two tools that request a clean shutdown. |
| `prompt_customizer.rs` | Tool and skill guidance added to the supplied system prompt. |

### Building an example

Authors need the pinned toolchain with the `wasm32-wasip2` target. Each example
builds to a component library; there is no command-line entry point:

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo build --locked \
  -p maestro-extensions-wasm --example <name> --target wasm32-wasip2 --release
```

### Running an example

The controlled runs instantiate the built component in the test host and call
the command and tools it registered:

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- hello_component_greets_through_the_host
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- shutdown_component_runs_every_registration
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- prompt_guidance
```

No provider, paid service or extension installation is required. Loading a
built component into an application is opt-in and needs that application's
component runtime.

### What the shutdown example does

`/quit`, `finish_and_exit` and `deploy_and_exit` ask the host to shut down
through their context, and each one keeps the answer it returns. The deployment
tool is illustrative: it reports progress, returns the environment it was given
and starts no deployment process. Loading the component into an interactive host
is opt-in and requests that host's shutdown.

### What the prompt guidance example does

`before_agent_start` returns a replacement `systemPrompt` built from the supplied
options: the prompt the host assembled, guidance for each active built-in tool in
read, bash, edit and write order, the supplied skill names in their original
order, and this extension's own context section. It discovers no resources and
builds no prompt.

The supplied prompt already holds the user append and any edit an earlier handler
made, so the example never appends that option again. Loading the component into
an application is opt-in.

# SDK Examples

Programmatic sessions through Maestro's embedding interface supply the model,
prompt, tools and extensions they need. The
[examples tracking issue](https://github.com/Orchestration-Maestro/maestro/issues/2074)
lists the standalone SDK examples still to come.

## Shipped support modules

These files belong to the [terminal RPC client](../rpc_extension_ui.rs). They
compile through its single registered entry point, not as separate examples.

| File | Purpose |
| --- | --- |
| `rpc_extension_ui/client.rs` | Retained widgets, streamed records, correlated commands and extension dialogs. |
| `rpc_extension_ui/output_log.rs` | Bounded output, literal model text and cell-aware viewport clipping. |

## Running

The [package index](../README.md) gives the client's build commands, companion
component and opt-in prerequisites. After those builds, run it from the package
root:

```sh
CARGO_TARGET_DIR=target CARGO_BUILD_JOBS=3 cargo run --locked \
  -p maestro-chat --example rpc_extension_ui
```

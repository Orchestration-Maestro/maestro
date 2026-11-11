# Examples

Extension, SDK and terminal RPC examples for Maestro. The extension and SDK
folders each have an index of their shipped files.

The default system prompt points the model at this folder. It sits beside the
package manifest, at `<package>/examples`, so the same path works in a source
checkout and in a standalone layout.

## Examples

| Path | Purpose |
| --- | --- |
| [extensions/](extensions/) | Extensions that register tools, commands and event handlers. |
| `rpc_extension_ui.rs` | A terminal RPC client with streaming output and extension dialogs. |
| [sdk/](sdk/) | Application-interface examples and the RPC client's support modules. |

## RPC extension UI

The client loads [rpc_demo.rs](extensions/rpc_demo.rs) and uses the
[client.rs](sdk/rpc_extension_ui/client.rs) and
[output_log.rs](sdk/rpc_extension_ui/output_log.rs) support modules.

This client is opt-in and needs a Unix or Windows terminal, the pinned Rust toolchain and
the `wasm32-wasip2` target. From the package root, build the executable and the
shipped companion before running the client:

```sh
rustup target add wasm32-wasip2
CARGO_TARGET_DIR=target cargo build --locked -p maestro
CARGO_TARGET_DIR=target cargo build --locked \
  -p maestro-extensions-wasm --example rpc_demo --target wasm32-wasip2 --release
CARGO_TARGET_DIR=target cargo run --locked \
  -p maestro-chat --example rpc_extension_ui
```

The launcher uses `target/debug/maestro` (`maestro.exe` on Windows) and
`target/wasm32-wasip2/release/examples/rpc_demo.wasm` beside the package manifest.
The commands above select that layout even when another Cargo target directory
is configured. The backend runs without saved session history and loads only
the shipped companion extension.

- `/select`, `/confirm`, `/input` and `/editor` demonstrate local dialogs. They
  send no model requests.
- `/rpc-input`, `/rpc-editor` and `/rpc-prefill` invoke the companion's input,
  multiline editor and editor-prefill commands over RPC, also without a model.
- Ordinary prompts use the configured model and normal tools. Provider
  credentials, model billing and file-changing tool effects are opt-in through
  those prompts. The companion's illustrative command check is not a sandbox.
- Esc dismisses a dialog, aborts a streaming run, or exits when idle. Ctrl+D
  quits. Terminal cleanup and child exit-status handling remain active on errors.

The client retains up to 1,000 output lines and shows the tail that fits the
terminal. Notification severity stays readable without color. Model text, tool
names and protocol errors cannot issue terminal control commands.

### Controlled checks

These use recorded terminal input, prepared native peers and an isolated loopback
provider, not live services or real credentials. The client's checks run the
shipped `rpc.py` example under Python 3; the component case also needs the target
installed above.

```sh
just prepare-tests
cargo nextest run --locked \
  -p maestro-chat --example rpc_extension_ui
cargo nextest run --locked \
  -p maestro --test integration -- rpc_demo_companion_
```

## Documentation

- [SDK examples](sdk/README.md)
- [Extension examples](extensions/README.md)
- [Writing an extension](../docs/extensions/guest.md)
- [Extension loading](../docs/extensions/loading.md)
- [Skills](../docs/resources/skills.md)

Every example file that does not ship yet is listed, with its owner, in
[the examples tracking issue](https://github.com/Orchestration-Maestro/maestro/issues/2074).

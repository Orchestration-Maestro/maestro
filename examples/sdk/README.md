# SDK examples

Use Maestro through an in-process session or a child process in RPC mode.
The [RPC integration guide](../../docs/rpc.md) describes the wire records;
the [Rust client](../../docs/client.md) provides typed subprocess operations.

| File | Purpose |
|---|---|
| `rpc.py` | Inspect session state or print one prompted run with an LF-only JSONL reader. |
| `test_rpc.py` | Check the Python example against disposable subprocess peers without provider calls. |

## Inspect a session

From the repository root, with Python 3 and Maestro on `PATH`:

```sh
python3 examples/sdk/rpc.py -- maestro --mode rpc --no-session --no-tools --offline --no-extensions --no-skills --no-prompt-templates --no-context-files
```

This sends `get_state` without a model request. The example prints protocol
records to stdout and leaves child diagnostics on stderr. To send plain prompt
text, add `--prompt TEXT` before `--`; choose a configured provider and model in
the child arguments. Prompting can incur provider charges. The example cancels
dialogs and stops after the first run ends; it does not track later automatic
retries or compaction recovery, or wait for slash commands handled without a run.

## Check the example

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s examples/sdk -p test_rpc.py
```

The tests cover LF framing with Unicode separators, startup dialogs, response
correlation, run completion, refusals, EOF and child exit status. They need
neither Maestro nor credentials.

For the interactive Rust flow, see the application-owned
[`rpc_client_demo`](../../crates/maestro-app/examples/rpc_client_demo/main.rs).
Other SDK examples are listed in the
[examples tracking issue](https://github.com/Orchestration-Maestro/maestro/issues/2074).

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

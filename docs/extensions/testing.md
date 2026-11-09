# Testing extensions

Test a built component on Maestro's production engine. The test harness loads it
through the artifact factory, registration runtime and runner the application
uses. History, credentials and the model catalog live in memory, and the catalog
serves the scripted model provider. A test reaches the extension only through the
runner's hook points and events, the ones the application calls. It needs no
model, network access or real credentials.

## Quick start

Start from the greeting project in the [extension guide](../extensions.md#create-the-project).
Add the engine from the same Maestro checkout, with its `test-support` feature, as
a development dependency:

```toml
[dev-dependencies]
maestro-extensions-wasmtime = { path = "../maestro/crates/maestro-extensions-wasmtime", features = ["test-support"] }
```

Put the test in `tests/hello.rs`. It loads the component the project builds and
calls the tool it registered:

```rust
use maestro_extensions_wasmtime::test_support::maestro_extensions::wrap_registered_tool;
use maestro_extensions_wasmtime::test_support::{Harness, HarnessOptions, local};
use serde_json::json;

const COMPONENT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/target/wasm32-wasip2/release/hello_extension.wasm"
);

#[test]
fn hello_greets_the_supplied_name() {
    local(async {
        let harness = Harness::new("/work", HarnessOptions::default()).unwrap();
        let loaded = harness.load(vec![COMPONENT.into()]).await.unwrap();
        assert!(loaded.errors.is_empty());
        let tools = loaded.runner.get_all_registered_tools();
        let tool = wrap_registered_tool(&tools[0], loaded.runner.clone());
        let result = (tool.execute)("call".into(), json!({"name": "Ada"}), None, None)
            .await
            .unwrap();
        assert_eq!(result.details, Some(json!({"greeted": "Ada"})));
    })
    .unwrap();
}
```

Build the component, then run the test natively:

```sh
cargo build --locked --target wasm32-wasip2 --release
cargo test --locked
```

The test loads the artifact the first command wrote and never compiles Rust into
a component, so rebuild the component after each source change.

## The harness

Everything lives in `maestro_extensions_wasmtime::test_support`, behind the
`test-support` feature. The module re-exports `maestro_extensions`, so the
runner, its events, contexts and records are named through this one dependency.

| Item | Purpose |
| --- | --- |
| `local(test)` | Runs an async test on a current-thread runtime inside a local task set. Loaded components keep their native work on that set, so every harness call runs inside `local`. |
| `Harness::new(cwd, options)` | Builds the session owners, registers the scripted provider, and creates the event bus and the registration runtime. `cwd` is the working directory the session and every context report. |
| `Harness::load(paths)` | Loads each component path into the shared runtime, then binds a runner over the ones that loaded. Returns `Loaded`. |
| `Loaded::runner` | The `ExtensionRunner` over the loaded components. |
| `Loaded::errors` | One `ExtensionLoadError` for each component whose factory failed, in supplied order. Later paths still load. |

`HarnessOptions` holds three settings, each with a default:

| Field | Default | Effect |
| --- | --- | --- |
| `models` | One model, `faux-1` | The scripted models, as `FauxModelDefinition` values. |
| `agent_dir` | Empty | The agent directory whose component cache the engine uses. Empty compiles each component without a cache. |
| `widgets` | `ToolkitWidgets` over an unpublished live theme | The host widgets the components build with. |

The harness exposes the real owners it built:

| Field | Owner |
| --- | --- |
| `cwd` | The working directory the session reports. |
| `session_manager` | The in-memory session history. Append entries before an event to give handlers history to read. |
| `model_registry` | The in-memory catalog. It holds the scripted provider as `faux`, with the key `faux-key` and every scripted model. |
| `faux` | The scripted provider's `FauxProviderRegistration`: its API, models and response queue. |
| `bus` | The event bus every loaded component shares. Its tails run on the local task set. |
| `runtime` | The registration runtime every loaded component shares. |

The engine reads one process-wide cache location, chosen on the first load in
the process. Pass the same `agent_dir` to every harness in one test binary.

### Observe the extension

A test drives the extension and reads its answers through the runner and the bus:

| To check | Use |
| --- | --- |
| A registered tool | `runner.get_all_registered_tools()`, then `wrap_registered_tool(&tool, runner.clone())` and call its `execute` with an argument document, an optional `Cancellation` and an optional update callback. |
| A registered command | `runner.get_command(name)`, then call its `command.handler` with the argument text and `runner.create_command_context()`. |
| An event handler | `runner.emit(&mut event)` with a typed event, or the dedicated calls such as `emit_tool_call`, `emit_input` and `emit_before_agent_start`. The returned value is the combined handler result. |
| A handler failure | `runner.on_error(listener)`. The listener receives each attributed failure; the returned closure removes it. |
| A bus message | `harness.bus.on(channel, listener)`. |
| A factory failure | `Loaded::errors`. |

### Unconfigured capabilities

The harness binds no application action. An action the test has not bound, such
as sending a message or reading the active tools, fails with the runtime's own
error: `Extension runtime not initialized. Action methods cannot be called during
extension loading.` To serve an action, bind it on the runner with recording
closures:

```rust
runner.bind_core(actions, context_actions, provider_actions)?;
```

`bind_core` replaces every binding at once. Build `actions` from
`Local::unbound_actions()` and replace only the fields the test serves.

### The scripted provider

Queue the replies the scripted provider returns, in order, before the extension
calls a model:

```rust
use maestro_extensions_wasmtime::test_support::FauxResponseStep;
use maestro_extensions_wasmtime::test_support::maestro_extensions::{
    FauxAssistantMessageOptions, faux_assistant_message,
};

harness.faux.set_responses(vec![FauxResponseStep::Message(Box::new(
    faux_assistant_message("Two open questions.", FauxAssistantMessageOptions::default()),
))]);
```

`harness.faux.state.call_count()` reports how many requests the provider
received. Dropping the harness removes the provider's registration.

## Examples

### A refused registration fails the load

A runtime that a reload replaced refuses every registration with the message it
was given. The extension's factory returns that refusal, and the loader reports
it as the component's load failure:

```rust
local(async {
    let harness = Harness::new("/work", HarnessOptions::default()).unwrap();
    harness.runtime.invalidate(Some("replaced by a reload".into()));
    let loaded = harness.load(vec![COMPONENT.into()]).await.unwrap();
    assert_eq!(
        loaded.errors[0].error,
        "Failed to load extension: replaced by a reload"
    );
    assert!(loaded.runner.get_all_registered_tools().is_empty());
})
.unwrap();
```

### Maestro's own greeting tests

The [greeting tests](../../crates/maestro-extensions-wasmtime/tests/hello_example.rs)
use this harness against the shipped [`hello`](../../examples/extensions/hello.rs)
example. They check its schema, every recorded name, invocation with an aborted
signal and an update callback, and the refused registration above. Inside the
Maestro checkout, they pass the run's prepared agent directory so the engine
reuses the code the run compiled.

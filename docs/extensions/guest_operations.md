# Native operations in components

Get an owned native-operation handle from `ExtensionAPI::native_io()`. The handle can
outlive the callback that obtained it.

## Wait

```rust
# use maestro_extensions_wasm::{ExtensionAPI, ExtensionResult};
# async fn example(api: &ExtensionAPI) -> ExtensionResult<()> {
api.native_io().sleep(std::time::Duration::from_millis(250), None).await?;
# Ok(())
# }
```

Pass an `AbortSignal` to cancel that wait. Cancellation returns exactly `Aborted`;
a signal already aborted fails before a native timer is armed. The host uses the
existing cancellable native delay, with no extra timeout. Durations retain seconds
and nanoseconds across the component interface; the native timer owns its resolution.

Completion, cancellation and dropping the future remove that operation's signal
subscription. Dropping the wait does not abort the caller's signal. A retained wait
continues to observe the signal it started with, even after its callback returned or
the session's current signal changed.

## Executable example

Build the native-operation witness as a component:

```sh
mise exec -- cargo build -p maestro-extensions-wasm --example native_io_component \
  --target wasm32-wasip2 --release --locked
```

The example registers `native-wait` and `native-retained-wait`. Integration tests
load that artifact through the production host; guest-authoring tests also run the
same forwarding code against controlled imports.

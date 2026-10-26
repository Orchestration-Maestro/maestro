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

Host-supplied signals retain their original resource when passed to another native
operation. Author-defined `SignalPort` implementations use a temporary host signal;
completion or drop removes the forwarding listener without aborting the author signal.

## Children and bytes

```rust
# use maestro_extensions_wasm::{ExtensionAPI, ExtensionResult};
use maestro_extensions_wasm::child_process::SpawnOptions;
# async fn example(api: &ExtensionAPI) -> ExtensionResult<()> {
let child = api.native_io().spawn(
    "git".into(), vec!["--version".into()], SpawnOptions::default(),
).await?;
let status = child.wait().await?;
# Ok(())
# }
```

Commands and arguments are literal: the host does not insert a shell or rewrite paths.
An omitted cwd or environment inherits the host's selection. A supplied environment is
the complete environment; an empty one stays empty. `SpawnOptions::stdio` independently
selects `Pipe`, `Ignore` or `Inherit` for stdin, stdout and stderr, with three pipes by default.
An unpiped descriptor has no handle.

`stdin()` returns an owned `Writable`. Each `write` sends the complete supplied bytes;
`end` closes input after earlier admitted writes. `stdout()` and `stderr()` return
independent `Readable` handles. A read waits for bytes or EOF, preserves arbitrary binary
data, and returns `None` only at EOF. Reading or closing one alias affects its shared
consumer; getting another handle does not reset or replay bytes.

`wait` observes the child's exit and both output-close phases without consuming output.
Numeric exit codes and signal termination remain distinct. `kill` controls the retained
child and returns false after its exit has been observed. Explicit PID signalling supports
negative process-group IDs on Unix; callers choose their own direct-child fallback.

Children and pipes may outlive the callback that created them. Closing an output releases
that consumer's queued bytes without killing the child. Dropping child controls does not
kill it; the process owner continues collection and reaps the child. Spawn, pipe and
signal failures remain catchable at their original operation without a diagnostic prefix.

## HTTP responses and raw bodies

```rust
# use maestro_extensions_wasm::{ExtensionAPI, ExtensionResult};
use maestro_extensions_wasm::http::Request;
# async fn example(api: &ExtensionAPI) -> ExtensionResult<()> {
let response = api.native_io().fetch(Request {
    url: "http://127.0.0.1:8000/data".into(),
    method: None,
    headers: Vec::new(),
    body: None,
    signal: None,
}).await?;
let _status = response.status();
if let Some(body) = response.body() {
    let _first_chunk = body.read().await?;
}
# Ok(())
# }
```

An omitted method selects GET. Request bodies retain absence, present-empty bytes
and arbitrary binary bytes. Header normalization, status text and response headers
come from the shared native HTTP client. `ok()` means status 200 through 299;
other statuses are responses, not fetch errors. Fetch adds no retry or timeout.

Headers arrive before the body completes. A 204 response has no body; an empty 200
response has a body that returns EOF. Getting `body()` again aliases the same
consumer, with no replay. Responses and bodies can outlive their creating callback.
A retained body also keeps the caller's cancellation forwarding alive. Raw reads
preserve NUL, malformed UTF-8 and split code points without decoding.

## Executable example

Build the native-operation witness as a component:

```sh
mise exec -- cargo build -p maestro-extensions-wasm --example native_io_component \
  --target wasm32-wasip2 --release --locked
```

The example registers `native-wait`, `native-retained-wait`, `native-child` and
`native-http`. Integration tests load that artifact through the production host; guest-authoring tests also run the
same forwarding code against controlled imports.

# Native operations and helpers in components

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

## Native helpers

Terminal text operations in `maestro_extensions_wasm::utils` receive an
`&ExtensionAPI` explicitly. They synchronously call the installed host's existing
terminal text owner; the component contains no text algorithms.

- `visible_width` returns terminal cells, not bytes. The host handles Unicode
  clusters, ANSI escapes and tabs.
- `normalize_terminal_output` normalizes Thai and Lao vowels, leaving ordinary
  newlines and tabs unchanged.
- `extract_ansi_code` takes a UTF-8 byte offset and returns the exact escape and
  its byte length. A position without a complete supported escape returns `None`.
- `is_whitespace_char` and `is_punctuation_char` report whether any supplied
  scalar belongs to the host's set. Whitespace includes U+FEFF, not U+0085.

Calls return the host's error rather than choosing a component-local fallback.

### Columns and callbacks

`wrap_text_with_ansi`, `truncate_to_width`, `slice_by_column`, `slice_with_width`
and `extract_segments` use terminal cells and keep native cluster and style
handling. Truncation defaults to `"..."` with no padding. Strict slicing excludes
a whole grapheme that crosses the right edge; segment strictness applies only to
the after side. Width-bearing results report the actual selected cells.

`apply_background_to_line` asks the native owner to pad the complete line, then
calls the supplied `FnOnce` exactly once with that string. It does not clip an
over-width line. The callback may borrow local state and runs before the helper
returns; neither a successful call nor an error retains it.

### Keys

`keys::Key` provides the named literals and forwards every modifier constructor
to the native key owner. `super_key` is the Rust spelling of the super modifier.
Key identifiers remain open strings.

Matching, parsing, printable decoding, repeat and release checks use native
terminal input handling. Optional keys and scalars return `None` when absent.
The enhanced keyboard flag is shared with the native host and other API handles;
setting it is a process-wide effect, not component-local state.

### Fuzzy matching

`fuzzy_match` returns the native match and score. `fuzzy_filter` accepts any
borrowed item slice and a borrowing `FnMut` getter, with no serialization bounds.
Empty and whitespace-only queries call no getter. Other queries project each item
once in input order; native ranking returns checked positions into the original
slice. Returned references retain item identity and stable score ties.

The slice's membership stays fixed during the call. Interior-mutable payloads
remain live: an earlier getter may edit what a later getter reads. Getters are
never retained, including on error. Cancellation policy stays with the caller.

### Metadata

`frontmatter::parse_frontmatter` returns a complete `FrontmatterValue` tree,
including scalar and sequence roots, nested nulls, signed zero, NaN and both
infinities. Numbers cross a typed floating-point boundary, not a JSON number.
Mappings enumerate canonical array-index keys first, then other keys in insertion
order. YAML parsing remains entirely native.

`strip_frontmatter` parses before returning the body. Both operations preserve
native parser causes and coordinates. Ordinary text is not trimmed; a present
header selects the native trimmed body and newline normalization.

### Conversation and output

`convert_to_llm` delegates application-message projection to the native session
owner. `compaction::utils::serialize_conversation` uses its existing transcript
serializer, including tool-result clipping and argument JSON spelling.

`tools::truncate` provides head and tail selection with complete accounting,
scalar-aware line clipping and size formatting. Missing limits select the host's
defaults: 2000 lines, 51200 bytes and 500 line characters. Size formatting accepts
the native floating-point domain, including signed zero and nonfinite values.

### Paths and installation identity

`tools::path_utils::{expand_path, resolve_to_cwd, resolve_read_path}` use the
native host's home and filesystem, not the component's environment. They keep
the native authored-path policy and existing-spelling fallback order.

`config::version(&api)` reads the application installation identity supplied at
host composition. It does not use the component package version. Native hosts
supply this value to `artifact_factory(path, installation_version)`.

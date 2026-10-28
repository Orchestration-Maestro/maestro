# Native operations and helpers in components

`ExtensionAPI::create_read_tool_definition(cwd, options)` returns a retained native
read definition. Construction does not read a file. Its metadata, schema,
execution and renderers come from the native tool owner. See [read](../tools-read.md)
for its file, image and output rules.

## Execute without an extension context

`ExtensionAPI::create_read_tool(cwd, options)` returns a `Tool`. Its `execute`
callback takes the call ID, JSON arguments, optional signal and optional progress
callback. It does not read a command or event context, even when called from one.

The executable keeps the native name, label, description, parameter schema,
preparation and execution mode. The parameter schema remains the shared
`ParameterSchema<SchemaCode>`, not a JSON string. Prompt fields and renderers
belong to definitions only.

## Select a built-in by name

`create_tool(name, cwd, ToolsOptions)` returns a context-free executable;
`create_tool_definition(name, cwd, ToolsOptions)` returns a registerable `ToolDef`.
`ToolDef` is the same `ToolDefinition` record used by individual factories.
Each member of `ToolsOptions` keeps its individual default and affects only the
selected tool. Selection is native-owned and accepts exactly `read`, `bash`,
`edit`, `write`, `grep`, `find` and `ls`. Other spellings return
`Unknown tool name: <original name>` without invoking supplied operations.
This restriction does not apply to custom names assigned before registration.

`create_coding_tools` and `create_coding_tool_definitions` return read, bash,
edit and write in that order. `create_read_only_tools` and
`create_read_only_tool_definitions` return read, grep, find and ls.
`create_all_tools` and `create_all_tool_definitions` return an `IndexMap` keyed
by `ToolName`, in `ALL_TOOL_NAMES` order. These forms delegate membership and
ordering to the native presets; they keep the same per-tool options and owned
callbacks as the individual factories.

## Wrap and register a read definition

```rust
use std::rc::Rc;
use maestro_extensions_wasm::{ExtensionAPI, ExtensionResult, ReadToolOptions};

fn register_read(api: &ExtensionAPI, cwd: String) -> ExtensionResult<()> {
    let mut read = api.create_read_tool_definition(cwd, ReadToolOptions::default())?;
    let native = read.render_call.take().ok_or("read call renderer absent")?;
    read.render_call = Some(Rc::new(move |args, theme, context, emitter| {
        let mut expanded = context.clone();
        expanded.expanded = true;
        native(args, theme, &expanded, emitter)
    }));
    api.register_tool(read)
}
```

The [compiled registration example](../../crates/maestro-extensions-wasm/examples/context_bridge/builtin_registered.rs)
replaces edit operations, wraps `execute` to report progress, and registers the
retained definition. It also selects the same definition from
`create_all_tool_definitions`. The production-host tests invoke preparation,
execution, cancellation and results through that registration, after the
constructing command returns.

Build its component with:

```sh
~/.local/bin/capped mise exec -- cargo build --locked -p maestro-extensions-wasm \
  --example context_bridge --target wasm32-wasip2 --release
```

This example is part of the `context_bridge` test component, not a standalone
program. Its custom file effects are observed by the native test driver. Native
matching, result formatting and per-file ordering are unchanged.

Pass the received render context when delegating. Its opaque `native_row` keeps
the original native environment, state and invalidation capability. Public row
observations may be changed on a clone. Call arguments, result options and the
supplied theme remain independent of those observations.

The previous component is exactly the one passed by the wrapper: a native
component, an authored replacement or none. Returning a native component lets
its next rendering reuse that native object. Wrapping it in another `Component`
does not pretend the wrapper is the native object. Calling a native renderer
without `native_row` returns `native tool renderer requires a native row` and
produces no component.

Result renderers receive `ToolRenderResultLike`. Its flattened `result` keeps the
existing `AgentToolResult` content, details and termination fields; `is_error`
keeps the independent rendering observation, including absence. Execution still
returns `AgentToolResult`.

## Replace file observations

`ReadToolOptions` has independent `auto_resize_images` and `operations` fields.
The image fitting default is enabled. Omitting `operations` uses native file
access. Supplying `ReadOperations` replaces only these observations:

| Callback | Result |
| --- | --- |
| `access(absolute_path)` | Success or a diagnostic that stops the read. |
| `read_file(absolute_path)` | Original bytes, as `Vec<u8>`. |
| `detect_image_mime_type(absolute_path)` | Optional MIME text; omitting the callback, returning `None` or returning an empty string selects text. |

Callbacks return `OperationFuture` with `DiagnosticErrorInfo` failures. The native
owner resolves paths against the construction directory, checks cancellation,
orders the observations, decodes text and applies image policy. The active model
is read from the supplied execution context, not captured during construction or
replaced by a later runner binding. Image fitting is independent of model image
support: a text-only model adds the native warning without disabling fitting.

A pre-aborted signal starts no file observation. An abort while an observation
is pending settles as `Operation aborted`; a later callback failure cannot replace
that outcome. Caller-defined signal ports retain their subscription for the call.

The definition and its callbacks may outlive the factory call and the callback
that constructed it. Keep the returned definition or its callback clones for as
long as they are needed; dropping the last consumer releases their native
resources. These operations require the native component host and a
`wasm32-wasip2` component. The in-process controlled adapter does not simulate
native factories.

## Replace write effects

`ExtensionAPI::create_write_tool(cwd, options)` returns a context-free executable;
`create_write_tool_definition(cwd, options)` returns a registerable definition with
its native metadata and renderers. Neither constructor performs file work or reads
a model. See [write](../tools-edit.md#write-tool) for native behavior.

`WriteToolOptions::operations` optionally supplies `WriteOperations`:
`mkdir(parent)` creates parents recursively, and `write_file(absolute_path, content)`
receives the original string. Both return `OperationFuture<'static, ()>` with
`DiagnosticErrorInfo` on failure. Omitting operations uses the local filesystem.

The native owner resolves paths, waits for mkdir to succeed before starting write,
and reports UTF-8 bytes, not characters. A failed mkdir starts no write. These
calls use the same native per-file mutation queue as other file-changing tools.
Pre-abort starts no effect; cancellation while an effect is pending stops the
remaining stages but cannot undo an effect that already completed.

## Replace edit effects

`create_edit_tool` and `create_edit_tool_definition` retain the native edit's
argument preparation. Its callback accepts `&mut Value`: parsing string-valued
`edits` updates retained assistant arguments, while folding a legacy `oldText` /
`newText` pair changes only the returned preparation. Validation uses that returned
value; saved history and later model requests keep the retained form. Preparation
performs no file observations or writes.

`EditToolOptions::operations` optionally supplies
`EditOperations`: `access(absolute_path)`, `read_file(absolute_path)` returning
original bytes, and `write_file(absolute_path, content)` receiving the native
replacement text. Omitting operations uses the local filesystem.

Callbacks return `OperationFuture` with the original `DiagnosticErrorInfo`, not a
display string. In particular, edit access formats a supplied text or numeric
`code`; without a code it keeps the original message. Read and write failures stop
at their reached stage. Matching, validation, diff construction, BOM and line-ending
policy remain with the [native edit owner](../tools-edit.md).

## Share native file ordering

`ExtensionAPI::with_file_mutation_queue(path, operation)` takes its place in the
same native queue used by edit and write, before its returned future is polled.
The callback starts after earlier operations on that file finish; other files
can proceed independently. Native canonical paths identify existing files,
including symlink aliases. Unresolved paths use native lexical absolute paths.

The callback returns `ExtensionFuture<'static, T>`. Its value stays in the guest:
`T` needs no serialization bound and can retain `Rc` identity. Callback errors
release the turn and reach the caller unchanged. Dropping a waiting future
releases its turn without calling the operation. This does not undo effects
that have already started. Do not call a queued edit or write from inside a
queue operation for the same file: that nested call would wait for its own
predecessor to finish.

## Intercept native shell execution

`create_bash_tool` and `create_bash_tool_definition` return the native shell's
executable and registerable forms. See [shell tools](../tools-process.md) for
output selection, cancellation, timeouts and process cleanup.

`BashToolOptions` keeps four independent controls. `operations` supplies the
existing `BashOperations`; otherwise the native local backend uses `shell_path`.
A supplied operation bypasses local shell selection. A nonempty `command_prefix`
is inserted before the command, separated by one newline. `spawn_hook` then
receives that command, cwd and a fresh environment, before progress or execution.
It may replace all three fields, emit synchronous events through its borrowed
emitter, or return an error that prevents execution.

Environment entries retain insertion order and `None` values through hooks and
supplied operations. Native process creation omits unset values; an unset key is
not the same as an empty string. The emitter cannot escape the hook. Callback
captures stay owned by the returned tool rather than the factory call.

Both factory forms forward the optional progress observer while native execution
is running. Shell output decoding, snapshot coalescing and artifact ownership
stay native. When final output is truncated, progress and final details retain
the same full-output artifact. The observer stays retained until execution ends
or its future is dropped.

Observer failures use the native event bus's error reporter and do not replace
the tool's result, including failures on the initial and final notifications.
Use a spawn hook or tool event to prevent command execution.

Supplied operation failures use the native shell's status formatter. An exact
`aborted` error appends `Command aborted` to the selected output. A `timeout:`
prefix appends `Command timed out after <seconds> seconds`, using the first
colon-delimited value after the prefix without parsing it as a number. These
failures keep any full-output artifact. Other error messages pass through
unchanged without adding the accumulated output.

```rust
use std::rc::Rc;
use maestro_extensions_wasm::{BashToolOptions, ExtensionAPI, ExtensionResult};

fn register_shell(api: &ExtensionAPI, cwd: String) -> ExtensionResult<()> {
    let options = BashToolOptions {
        spawn_hook: Some(Rc::new(|mut spawn, _| {
            spawn.env.insert("UNSET_IN_CHILD".into(), None);
            Ok(spawn)
        })),
        ..BashToolOptions::default()
    };
    api.register_tool(api.create_bash_tool_definition(cwd, options)?)
}
```

## Run raw local shell operations

`ExtensionAPI::create_local_bash_operations(shell_path)` retains the native local
backend. Construction starts no process. The application's binary directory and
the optional executable reach that backend without a second shell implementation.

Its `exec(command, cwd, options)` callback receives owned operands. `on_data`
receives raw stdout and stderr bytes: no UTF-8 decoding, ANSI stripping, binary
filtering or newline conversion runs in the guest. A failing raw observer is
reported through the native event bus; execution still settles normally.

The optional signal stays live until execution ends, including when operations
outlive the event that returned them. An absent signal remains absent. Positive
`timeout` values are seconds; absent and nonpositive values add no deadline.
Exit status, cancellation, timeouts and process cleanup remain native-owned.

Return these operations from a `user_bash` handler to replace interactive shell
execution. The session accepts Local operations and applies its usual output
sanitization before recording and painting them. Direct `exec` calls still receive
raw bytes. Supplying the same operations to `BashToolOptions` retains the native
tool's truncation, artifact and status formatting.

The executable [local shell author example](../../crates/maestro-extensions-wasm/examples/local_shell_component.rs)
shows direct execution and interactive interception through the same facade.

## Replace search observations

`create_find_tool` / `create_find_tool_definition` and `create_grep_tool` /
`create_grep_tool_definition` return the native factories' executable and
registerable forms. Construction does not inspect files or acquire programs.
See [search tools](../tools-search.md) for native search and output behavior.

`FindToolOptions::operations` supplies `exists(absolute_root)` and
`glob(pattern, absolute_root, FindGlobOptions)`. Glob options retain the ordered
ignore patterns and effective floating-point limit. Return ordered relative or
absolute paths; the native finder resolves them and selects output and notices.

`GrepToolOptions::operations` supplies `is_directory(absolute_root)` and
`read_file(absolute_file)` returning context text. Matching still uses native
ripgrep, including when these observations are replaced. Root errors and unreadable
context retain the native search owner's handling.

All four callbacks return `OperationFuture` with `DiagnosticErrorInfo` on failure.
The host retains them independently of the constructor and invokes them without a
callback-table borrow. Omitted operations select native filesystem access.

Native applications install `ToolConstruction` with `set_tool_construction`
before loading extension factories. It carries the same `Arc<ToolsManager>` and
binary directory used by the application's own tools. The SDK supplies these
services during resource loading and retains them on reload; the component runtime
never discovers a default directory or creates another manager. An unconfigured
host returns a construction error.

## Replace directory observations

`create_ls_tool` and `create_ls_tool_definition` return the native listing owner's
executable and registerable forms. `LsToolOptions::operations` supplies
`exists(absolute_root)`, `stat(absolute_path)` and `readdir(absolute_root)`.
`stat` returns whether the root or entry is a directory; `readdir` returns entry
names in producer order. All three callbacks return `OperationFuture` with
`DiagnosticErrorInfo` on failure.

Construction performs no directory access. The host retains supplied callbacks;
omitted operations use the native filesystem. Sorting, directory suffixes,
per-entry stat failure skipping, limits and output notices stay with the
[native listing owner](../tools-search.md).

Get an owned native-operation handle from `ExtensionAPI::native_io()`. The handle can
outlive the callback that obtained it.

These resources leave the standard full-user environment, filesystem and standard
streams intact. Components can use Rust's standard library to read inherited
environment values, read and write absolute files outside cwd, and follow relative
symlinks allowed by the host. Standard input, output and error remain separate byte
streams. The default streams are shared with the native process: reading stdin or
writing stdout can interfere with a protocol frontend. The native-operation handle
does not change their routing.

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

`text().await` consumes the same body once through the native HTTP collector and
text decoder. It strips one initial UTF-8 BOM and replaces malformed UTF-8; it does
not trim whitespace or remove NUL. `json().await` uses that decoder and the native
JSON parser, returning any JSON value, including null, scalars and signed zero.
Finite numbers and named object members cross the existing lossless JSON wire.
Empty or malformed JSON fails at consumption, not at fetch.

Getting a body handle does not consume it. A raw read prevents later text or JSON
collection; a completed or failed collection cannot be replayed by either method.
Collection and raw reads observe the original request signal after headers arrive.
A late body error leaves already delivered bytes intact. Closing or dropping the
last body consumer releases its work without aborting the caller signal.

## File watches

```rust
# use maestro_extensions_wasm::{ExtensionAPI, ExtensionResult};
# fn example(api: &ExtensionAPI, path: String) -> ExtensionResult<()> {
let watch = api.native_io().watch(path, std::rc::Rc::new(|emitter| {
    emitter.emit("file-changed", serde_json::Value::Null)
}))?;
watch.close()?;
# Ok(())
# }
```

`watch` passes the supplied path to the shared native watch owner, including an
absolute file outside the current directory. Notifications invoke the retained
callback with its synchronous emitter. They do not promise an event count or
ordering, and the bridge adds no polling or debounce.

A watch can outlive its creating callback. Cloning its author handle keeps the
same registration alive. Explicit close, repeated close and dropping the last
handle stop delivery and release its callback. A callback may close its own watch;
its active invocation finishes before its final captures are released. A creation
failure returns the native error without retaining the callback or stopping a
sibling watch.

## Settings and packages while loading

`SettingsManager::new(api)` and `DefaultPackageManager::new(api)` call the host's
own settings and package owners. They work from the factory onwards, before any
session exists, and again on every reload. The component reads no settings file
and runs no Git or npm command itself.

- `get_global_settings` and `get_project_settings` return the raw user and project
  documents. Unknown keys and wrongly typed values stay as written, and the two
  documents are not merged.
- `resolve` lists the configured packages; when both scopes name one identity, the
  project's entry wins. `resolve_extension_sources(sources, scope)` resolves sources
  the extension chose itself, in one scope, and never writes them to settings. Each
  typed `ResolvedPackage` descriptor holds `source`, `identity`, `scope` and
  `installed_path`. A package without resources is listed too.
- `install(source, scope)` acquires a source at its pin without editing settings.
- `verify_pin(source, scope)` returns a typed `GitPinStatus` with
  `expected_commit`, `head_commit`, `changed_paths` and `is_clean()` for a pinned
  Git checkout and changes nothing.
  `restore_pin(source, scope)` returns the checkout to its pinned commit and removes
  changed and untracked files.

`OperationScope` is `User` or `Project`. Offline mode, progress and the configured
Git and npm commands belong to the package owner. A failure returns the host's
message, for example `Integrity requires a pinned Git source` for a local source.
Native factories make the same calls with `ExtensionAPI::host_operation` and a
`HostOperation` request.

## Executable example

Build the native-operation witness as a component:

```sh
mise exec -- cargo build -p maestro-extensions-wasm --example native_io_component \
  --target wasm32-wasip2 --release --locked
```

The example registers `native-wait`, `native-retained-wait`, `native-child`,
`native-http` and `native-watch`. Integration tests load that artifact through
the production host; guest-authoring tests also run the same forwarding code
against controlled imports.

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
`tools::path_utils::basename(&api, path)` returns the final path component using
the host's path flavor, preserving literal backslashes on POSIX hosts.

`tools::path_utils::resolve(&api, &[cwd, path])` resolves operands lexically
with the host platform's path rules. It reduces `.` and `..` segments and
trailing separators without filesystem lookup, Unicode-space replacement or
`~`/`@` expansion.

`tools::path_utils::guest_path(&api, path)` spells a native host path the way the
component's standard library opens it. Windows drive and share paths change,
Unix paths do not; see [the operating environment](host_io.md).

`config::version(&api)` reads the application installation identity supplied at
host composition. It does not use the component package version. Native hosts
supply an `Installation { version, agent_dir }` to
`artifact_factory(path, widgets, installation)`.

`config::get_agent_dir(&api)` reads the agent directory the application supplies
when composing the component host. Like `config::version(&api)`, it reads the
host's installation facts, not the component's inherited environment.

## Read the host's local clock

`utils::local_date_time_string(&api)` reads the host's current wall clock and local
time zone. It returns the en-US pattern `M/D/YYYY, h:mm:ss AM`, including seconds,
with unpadded month, day and hour fields on a twelve-hour clock. It does not select
other locale patterns. The guest carries no time-zone data; host failures remain errors.

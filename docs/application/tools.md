# Application tools

## Quick start

Create executable tools through `maestro_app`. The application exports the existing
native factories; it does not add another execution layer.

```rust
use maestro_app::{ReadToolOptions, create_read_tool};

let read = create_read_tool("/project".into(), ReadToolOptions::default());
assert_eq!(read.definition.name, "read");
```

These factories are native-only. Browser execution uses the
[browser tools](browser-tools.md).

## Using tools

The root exports `create_read_tool`, `create_write_tool`, `create_edit_tool`,
`create_bash_tool`, `create_grep_tool`, `create_find_tool` and `create_ls_tool`.
Each returns an executable agent tool. Calls accept authored paths relative to
the supplied working directory or absolute paths. File and process operations
remain replaceable through the corresponding options.

- [Read](../tools-read.md) selects text lines, reports continuation offsets and
  identifies images by their bytes.
- [Write and edit](../tools-edit.md) preserve UTF-8 file content. Write success
  counts bytes, not characters. Edit matches every replacement against the
  original file, rejects ambiguous or overlapping regions before writing, and
  retains the file's line endings and BOM.
- [Shell execution](../tools-process.md) streams raw bytes to tool output. A
  truncated result includes a readable full-output artifact when the call settles,
  including timeout and cancellation failures.
- [Search and listing](../tools-search.md) retain their native search manager,
  ignore rules and output limits.

See [native factories](../tools.md) for presets, typed definitions and renderers.

## Direct shell history

`AgentSession::execute_bash` runs a direct command and records a `bashExecution`
message. It is separate from an agent-invoked bash tool and its `toolResult`.
`abort_bash` cancels the active direct command; `is_bash_running` stays true until
execution settles.

`record_bash_result` records an externally obtained result without launching a
command. An idle session appends it to live history and stored entries at once.
While a response streams, the session keeps it pending. Finishing that response
does not flush it: the next prompt appends it before the next user message.
Shell history records emit no `message_end` event.

## Customizing qualification

The [application test harness](testing.md) supplies complete sessions with a
scripted provider and disposable files. File tests call the root factories and
compare complete results, diffs and written bytes with recorded fixtures.
Controlled shell operations supply raw byte chunks and settlement outcomes;
the real tools own decoding, progress coalescing, truncation and artifact closure.
Artifact checks run in checked Rust children with disposable temporary storage.

The persistence checks retain the same session across an acknowledged waiting
tool, shell recording and the following prompt. They compare pending state,
live message order and stored entry order at each boundary. Separate checks
retain cancelled shell results and interrupted assistant messages.

## Examples

Run the executable file and controlled-shell checks:

```sh
cargo nextest run -p maestro-app --locked --test integration \
  -E 'test(/^tools_(read|write|edit|bash|search)_checks::/)'
```

Run direct-shell persistence checks:

```sh
cargo nextest run -p maestro-app --locked --test integration \
  -E 'test(/^agent_session_bash_persistence_checks::/)'
```

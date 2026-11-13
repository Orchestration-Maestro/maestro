# Application tools

## Quick start

Create executable tools through `maestro_app`. The application root keeps the
native factories from `maestro-tools` as part of the embedding interface; a
frontend imports them from `maestro_tools`.

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
While a run streams, the session keeps it pending. When that run settles, the
session appends pending results after its tool calls and results, even if no later
prompt follows. A later idle shell result follows those earlier results.
Shell history records emit no `message_end` event.

## Customizing operations

Supply the tool's operations interface through its options to replace file or
process effects. Keep the tool factory: it retains argument handling and output
formatting around those operations. For example, `ReadToolOptions::operations`
accepts `ReadOperations`, and `BashToolOptions::operations` accepts
`BashOperations`. See [native factories](../tools.md) for the interfaces and
[application testing](testing.md) for controlled operation adapters.

## Examples

Use an offset and limit to [read part of a file](../tools-read.md), or supply
multiple replacements to [edit a file atomically](../tools-edit.md). For an
embedded session, [select tools before prompting](sdk.md#limit-tools-before-prompting).

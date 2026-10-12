# File-changing tools

`maestro-tools` delivers the native `write` tool and the file mutation queue that
orders every file-changing tool. The `edit` tool and its diff helpers continue in
[#690](https://github.com/Orchestration-Maestro/maestro/issues/690),
[#691](https://github.com/Orchestration-Maestro/maestro/issues/691) and
[#692](https://github.com/Orchestration-Maestro/maestro/issues/692); they are not
part of this crate yet.

Both items are native-only: the `write` module and the queue are excluded from
browser builds, which still build the portable tool definitions.

## Write tool

[`create_write_tool_definition`](../crates/maestro-tools/src/write/mod.rs) builds the
`write` definition and `create_write_tool` wraps it as an `AgentTool` with no caller
context. The model supplies two required strings:

| Argument | Meaning |
|---|---|
| `path` | File to write, relative to the tool's working directory or absolute. |
| `content` | Complete new content of the file. |

Other members of the argument object are accepted and ignored. A missing `path` or
`content`, or an object or array in its place, fails the shared argument validation
before any file operation; a number or boolean is converted to text by that
validation. No preparation callback, execution-mode override or shell override is
declared.

Path resolution is the first step of execution: a leading `@`, a leading `~` and
Unicode spaces are expanded by [`resolve_to_cwd`](../crates/maestro-tools/src/path_utils.rs),
and the working directory is read only when the path and the tool's `cwd` cannot
anchor the result. The path is resolved when the tool is called, before its future
is first polled, so a later change of the process working directory does not move
the write.

A successful call creates the missing parent directories and replaces the file,
including with empty content. It returns one text block, `Successfully wrote N bytes
to <path>`, where `N` is the number of UTF-8 bytes written (`€` counts 3, `😀` counts
4) and `<path>` is the path as the model wrote it. There are no result details and no
progress events. A failure of directory creation or of the write is returned
unchanged, with its operating-system error number when it has one, and a failed
directory creation prevents the write. The tool performs no permission check, atomic
replacement or rollback.

```rust
use maestro_tools::{WriteToolOptions, create_write_tool};

let tool = create_write_tool("/work".into(), WriteToolOptions::default());
assert_eq!(tool.definition.name, "write");
```

### Replacing the file access

`WriteOperations` is the seam between the tool and the files. `write_file` stores
UTF-8 content at an absolute path and `mkdir` creates a directory with its parents.
`WriteToolOptions::operations` replaces the native filesystem for one tool, for
example to write through another system; the tool resolves paths and orders the
calls the same way for every implementation, and calls `mkdir` with the parent of the
resolved path before `write_file`.

### Cancellation

A call that is already cancelled when it reaches the front of its file's queue
settles as `Operation aborted` without calling any operation. Directory creation and
the write are each cancellable: when the signal fires first, or fires while an
operation runs, the call settles as `Operation aborted`, drops the operation's
pending future, never starts the next stage and lets the next queued call for the
file proceed. Cancellation does not undo an effect that already happened, and a
native filesystem write that has already been handed to the runtime can still finish.
A call that waits behind another call for the same file observes its signal only
after that call has finished.

### Previews

The call renderer draws the tool name, the path and a preview of the content as it
streams in: ten lines when collapsed, all of them when expanded, followed by the
count of hidden lines and the key bound to expanding. The language is taken from the
file name alone (`file_path` is accepted as a display alias of `path`; an empty alias
shows `...` instead of the path); an unrecognized name stays plain. While the received
content only grows, only the changed last line and the new lines are highlighted
again, and the first 50 lines are refreshed together; any other change highlights it
all again. The result renderer draws nothing for a success and the error
text for a failure. Display strings, tab expansion and key hints follow the
[display helpers](tools-read.md#display-helpers). Rendering never writes a file.

## File mutation queue

[`with_file_mutation_queue`](../crates/maestro-tools/src/file_mutation_queue.rs)
runs an operation after every earlier call for the same file has finished.

- A call takes its place when `with_file_mutation_queue` is called, not when the
  returned future is first polled, so calls run in the order they were made.
- A file is identified by its canonical path; when the path cannot be canonicalized,
  by its lexical absolute path. Symlink aliases of one file share a queue, hard links
  with different names do not, and different files never wait for each other. The
  path is used as written: `@`, `~` and Unicode spaces are not expanded.
- An operation that fails does not block the calls behind it.
- Dropping a returned future leaves the queue without running its operation, and
  does not let a later call run before the call that is active.
- When the last call for a file finishes or is dropped, the file has no queue entry.

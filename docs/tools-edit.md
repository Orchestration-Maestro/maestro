# File-changing tools

`maestro-tools` delivers the native `write` and `edit` tools, the file mutation queue,
original-snapshot edit matching, numbered diffs and read-only edit previews.
The portable shared diff renderer is available separately. Native edit definitions
retain boxed call previews and a separate result component.

The `write` and `edit` modules, the queue and `edit_diff` are native-only and
excluded from browser builds, which still build the portable tool definitions.

## Write tool

[`create_write_tool_definition`](../crates/maestro-tools/src/write/mod.rs) builds the
`write` definition and `create_write_tool` wraps it as an `AgentTool` with no caller
context. The model supplies two required strings:

| Argument | Meaning |
|---|---|
| `path` | File to write, relative to the tool's working directory or absolute. |
| `content` | Complete new content of the file. |

Other members of the argument object are accepted and ignored. Missing or
mistyped `path` and `content` follow the shared argument validation. No preparation callback,
execution-mode override or shell override is declared.

Path resolution is the first step of execution and follows
[`resolve_existing_path`](../crates/maestro-tools/src/path_utils.rs); the working
directory is read only when the path and the tool's `cwd` cannot anchor the result.
A path naming an existing file resolves to that file, so a rewrite changes the file
that carries the written spelling; a path naming a new file follows
[the shared spelling rule](tools-read.md#read-tool). The path is resolved when the
tool is called, before its future is first polled, so a later change of the process
working directory does not move the write.

A successful call creates the missing parent directories and replaces the file,
including with empty content. It returns one text block, `Successfully wrote N bytes
to <path>`, where `N` is the number of UTF-8 bytes written (`€` counts 3, `😀` counts
4) and `<path>` is the path as the model wrote it. There are no result details and no
progress events. A failure of directory creation or of the write is returned
unchanged unless cancellation fires first, with its operating-system error number when it has one, and a failed
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
file proceed. Cancellation does not undo an effect that already happened. A native
filesystem write that has already been handed to the runtime can still finish, and the
file's queue stays held until it does, so the next call never overlaps it.
A call that waits behind another call for the same file observes its signal only
after that call has finished.

### Previews

The call renderer draws the tool name, the path and a preview of the content as it
streams in: up to ten lines when collapsed, all of them when expanded, followed, when lines are hidden, by the
count of hidden lines and the key bound to expanding. The language is taken from the
file name alone (`file_path` is accepted as a display alias of `path`; an empty alias
shows `...` instead of the path); an unrecognized name stays plain. While the received
partial content only grows, the changed last line and the new lines are highlighted
again and the first 50 lines are refreshed together; unchanged partial content does
nothing, and complete arguments or any other change highlight it all again. A path
with no recognized language is not highlighted. The result renderer draws nothing for a success and the error
text for a failure. Display strings, tab expansion and key hints follow the
[display helpers](tools-read.md#display-helpers). Rendering never writes a file.

## Edit tool

[`create_edit_tool_definition`](../crates/maestro-tools/src/edit/mod.rs) builds the
native definition; `create_edit_tool` wraps it without caller context. Required
arguments are `path` and `edits`, an array of `{ "oldText": "…", "newText": "…" }`
replacements. Both object levels reject additional properties during shared
validation. Both rendering callbacks share an opaque `EditRenderState`.

### Retained edit rendering

The call renderer prefers its previous edit component, otherwise the component in
row state, otherwise a new padded box. Complete preview inputs use the render
context's working directory and supplied [terminal runtime](terminal/rendering.md),
not the definition's execution operations. Preview selection accepts an ordered
nonempty array of string pairs, or a complete top-level string pair; it does not
run argument preparation. Display aliases follow the [display helpers](tools-read.md#display-helpers).

Only the current outstanding preview may publish; any result revokes that request.
A successful result with details rebuilds an existing call preview when its diff
or first changed line changes, without requiring complete arguments or a file
read. An execution error does not erase an existing preview. Results suppress
ordinary success text and duplicate preview text; detached results reuse their
own container. Diff styling belongs to the [diff renderer](#diff-rendering) and
padding and wrapping belong to the [terminal components](terminal/components.md).

The supplied runtime owns preview futures. The invalidator must not strongly
retain its owning runtime or writer; use a weak render request and weak row
ownership. Dropping the last call component prevents its pending preview from
publishing or requesting a redraw.

### Preparation and execution

Preparation parses string-valued `edits` in the retained call only when the JSON
result is an array. A complete top-level string pair `oldText`/`newText` is
appended to the returned array (or a new array), removing those two keys only
from the returned arguments and retaining other members. Partial
pairs remain unchanged after array parsing. Hooks and progress observe this
in-place normalization, as described in [conversation operations](loop.md).
Preparation runs before the
[shared argument validation](models/arguments.md) in the agent loop and performs
no file operations. Direct execution takes already prepared arguments.

Empty lists fail before path resolution or queue admission. Other calls resolve
through [`resolve_existing_path`](../crates/maestro-tools/src/path_utils.rs); successful
resolution enters the
[file mutation queue](#file-mutation-queue) before polling.
Execution checks read/write access, reads bytes lossily as UTF-8 and uses the
[matching owner](#original-snapshot-matching) against one original snapshot.
After restoring the retained prefix BOM and detected line endings, it writes
and returns one text block, `Successfully replaced N block(s) in <path>.`, using
the requested count and authored path. Present `EditToolDetails` holds the LF
numbered diff and optional `firstChangedLine`; no progress or termination is
requested. Matching failures do not write.

`EditToolOptions::operations` selects an `EditOperations` implementation with
`access`, raw-byte `read_file` and UTF-8 `write_file`. Omission selects the existing
native adapters. Supplied operations use the same execution callback; no image
sniffing, filename fallback or parent-directory creation is added.

Access, read and write each follow the [cancellation policy](#cancellation):
pending supplied futures are dropped, admission is released and completed
effects are not undone. Native work already submitted may finish. A cancelled
waiting call observes cancellation only after its predecessor finishes.

## Original-snapshot matching

[`edit_diff`](../crates/maestro-tools/src/edit_diff.rs) owns matching and previews.
The pure matching operation `apply_edits_to_normalized_content` reads no file; it takes
content that already uses LF endings (convert it with
[`normalize_to_lf`](text.md)) and returns the comparison base with the edited text, or a
diagnostic.

- Operands: `old_text` and `new_text` are converted to LF; an empty `old_text` is
  rejected before any matching. Replacement text is inserted literally.
- Snapshot: every edit is matched against the same text, never against another
  edit's output. Matches are sorted by position, must not overlap (touching is fine)
  and are then spliced in one pass.
- Fuzzy base: matching tries the exact text first. If any edit matches only after
  NFKC, per-line trailing-whitespace removal and quote, dash and space folding,
  every edit is matched against that normalized text, which is returned as the base.
- Uniqueness: occurrences are counted in normalized space, overlaps included, even
  when the chosen match is exact. A nonempty `old_text` that normalization erases
  matches and counts literally only; it is never an empty insertion point.
- Positions and lengths are UTF-8 byte offsets into the text they describe.
- Trailing whitespace follows the shared set of [text](text.md).
- `detect_line_ending` follows the first LF, `restore_line_endings` turns each LF into
  CR LF in CRLF mode, and `strip_bom` removes one leading BOM.

The nine failure messages are the exact texts in the module; single-edit calls use
the unnumbered wording; multi-edit calls name `edits[n]` except for the no-change
failure, which names no edit.

## Numbered diffs and read-only previews

[`generate_diff_string`](../crates/maestro-tools/src/edit_diff/numbered.rs) returns
numbered changes and the first changed line in the new text. Omitted context selects
four lines; explicit context includes zero. Identical texts return an empty diff
and no first changed line.

[`compute_edits_diff`](../crates/maestro-tools/src/edit_diff/preview.rs) checks
readability, reads the target and returns a diff or caught failure without writing it.
`compute_edit_diff` delegates one replacement to that operation. Path resolution
uses [`resolve_existing_path`](../crates/maestro-tools/src/path_utils.rs) when called;
resolution failures are returned before a future is created. Access, read and
matching failures are returned by the future. Access failures retain the authored
path and supplied error code, including native numeric codes; read and matching
failures retain their messages.

Preview decodes bytes lossily, strips one leading BOM and normalizes newlines,
then uses the [matching operation's](#original-snapshot-matching) comparison base.

## Diff rendering

[`render_diff`](../crates/maestro-tools/src/diff/mod.rs), also exported from the
application root, colors numbered diff rows with the supplied
[live theme](theme.md#live-theme-and-change-callback). Only a consecutive run of one removed row
followed by one added row receives word-level inverse styling. Other rows keep
context, removed or added foregrounds. Recognized content tabs become three spaces;
unrecognized rows remain raw. Inverse boundaries stay outside escapes recognized
by the [terminal text utilities](terminal/text.md).

`RenderDiffOptions::file_path` is accepted without selecting highlighting or reading
files. The renderer does not compute edits, wrap text or truncate rows.

## File mutation queue

[`with_file_mutation_queue`](../crates/maestro-tools/src/file_mutation_queue.rs)
runs an operation after every earlier call for the same file has finished.

- A call takes its place when `with_file_mutation_queue` is called, not when the
  returned future is first polled, so calls run in the order they were made.
- A file is identified by its canonical path; when the file or one of its parent
  directories does not exist yet or a parent is a regular file, by its lexical absolute
  path. The operation then reports its own access or write failure. Any other
  canonicalization failure, such as a symlink loop or denied permission, settles the
  call before its operation runs. Symlink aliases that canonicalize share a queue, hard links
  with different names do not, and different files never wait for each other. The
  path is used as written: `@`, `~` and Unicode spaces are not expanded.
- An operation that fails does not block the calls behind it.
- Dropping a returned future before its operation has started leaves the queue
  without running it; an operation already started is not undone. Dropping does not let a later call run before the call that is active.
- Blocking filesystem work that an operation started keeps the file's queue held until
  it returns, even when the call's future is dropped first.
- When the last call for a file finishes or is dropped, the file has no queue entry.

# Session history and stored-entry context

`maestro-session::session_manager` parses supplied JSONL text and projects a
selected ancestry into context. Native operations create, reopen, switch and
continue JSONL histories; explicit memory history remains available.
Stored timestamp conversion delegates to [message formatting](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/sessions/messages.md).

## Raw entries and explicit views

`FileEntry` and `SessionEntry` are ordered JSON values, not validated schemas.
`parse_session_entries` removes outer ECMAScript whitespace, splits at LF, skips
blank or malformed lines and accepts native JSON values without requiring a
header. Nonblank interior lines are parsed without extra trimming. Native JSON
numeric and nesting limits apply. Object members keep their first position with
the last duplicate value; omitted and null properties remain distinct.

The named header and entry records implement standard serde views. Decode from
`&Value` only when those fields are needed; raw parsing and projection do not
validate unused payloads. Context messages and settings remain raw, owned values.

```rust
use maestro_request::presence::Presence;
use maestro_session::{parse_session_entries, build_session_context};
use std::collections::HashMap;

let entries = parse_session_entries(
    r#"{"type":"message","id":"a","parentId":null,"message":{"role":"user","content":"hello","timestamp":1}}"#,
);
let context = build_session_context(
    &entries, Presence::Missing, None::<&HashMap<_, _>>,
)?;
assert_eq!(context.messages[0]["content"], "hello");
let empty = build_session_context(
    &entries, Presence::Null, None::<&HashMap<_, _>>,
)?;
assert!(empty.messages.is_empty());
# Ok::<(), serde_json::Error>(())
```

## Branch selection

Without a supplied index, indexing runs before explicit-null selection and the
last entry with a given ID owns lookup. A supplied index is authoritative, even
when empty, and may refer to entries outside the slice. Explicit null selects
before-first; missing, empty or unknown leaf IDs fall back to the last supplied
entry. A missing parent stops the reachable ancestry. Revisiting the same entry stops
before adding it again, retaining the reachable suffix rather than looping.

Settings are observed across the selected root-to-leaf path before message
emission. Later thinking and model observations overwrite earlier ones. The
latest path compaction emits its summary, its selected precompaction kept range,
then later messages. Ordinary messages and hidden custom messages contribute;
custom state, labels, session names and unknown entry kinds do not. Branch
summaries contribute when their raw summary is truthy.

Generated messages convert timestamps during projection through the existing
[message formatting](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/sessions/messages.md) owner. Explicit typed message decoding and
model conversion remain separate operations. Reached null entry or message
receivers return native errors identifying the field, not an eager schema error.

`get_latest_compaction_entry` scans supplied order backwards without ancestry
selection and borrows the matching original element. It stops at its first match;
a null entry encountered before that match is an error.

## In-memory history

`SessionManager::in_memory` creates a memory history with no session-file reads
or writes. An omitted cwd captures the process working directory and propagates
its native lookup error; an explicit string, including empty, stays as authored.
Session identities use `UUIDv7` unless `NewSessionOptions.id` supplies a string.
Headers and message entries receive fresh UTC millisecond timestamps. Entry
identities try eight hexadecimal characters from a random UUID up to 100 times
before falling back to a full UUID.

Appending moves the raw message into the retained sequence, indexes it and
selects it as a child of the previous selection. `branch` validates membership
before selecting; a missing target returns exactly `Entry {id} not found`.
Abandoned branches remain in history. `get_branch(None)` uses the selection;
an explicit empty or unknown identity returns an empty path without changing it.
Context uses the [projection rules](#branch-selection) above.

Reads borrow retained records. To keep a record across mutable operations,
callers take an explicit owned copy. `new_session` replaces the header and
clears entries, index and selection while retaining cwd. It writes no file;
native history selects a fresh filename, while memory history retains any
explicitly selected filename.
`NewSessionOptions.parent_session` preserves missing, null and string metadata.
Memory history reports false persistence and an empty session directory. Ordinary
memory operations perform no file I/O; explicit native `set_session_file` may
read and convert records without writing them. `get_entries` borrows every
nonheader record in retained order.

```rust
use maestro_session::{NewSessionOptions, SessionManager};
use serde_json::json;

let mut history = SessionManager::in_memory(Some("work"))?;
let root = history.append_message(json!({"role":"user", "content":"hello", "timestamp":1}))?;
history.append_message(json!({"role":"user", "content":"abandoned", "timestamp":2}))?;
history.branch(&root)?;
history.append_message(json!({"role":"user", "content":"selected", "timestamp":3}))?;
let context = history.build_session_context()?;
assert_eq!(context.messages[1]["content"], "selected");
assert_eq!(history.get_entries().len(), 3);
assert_eq!(history.new_session(NewSessionOptions::default()), None);
assert!(history.get_entries().is_empty());
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Native history

Native `create` and `continue_recent` use the supplied directory or encode the
stored cwd beneath the caller's resolved agent root. `open` resolves its explicit
filename lexically and reads it once; cwd override wins over stored cwd, with a
runtime fallback for missing/null cwd. Explicit directory spellings are retained.

A fresh history publishes at its first assistant message. Loaded and recovered
histories stay published: subsequent appends write one record. Append errors leave
accepted memory and any partial bytes in place; failed switches retain the prior
manager state. Empty or invalid-header files are rewritten immediately at the
selected path. A missing explicit file waits for first publication at that path.

Continuation filters valid `.jsonl` files using a first-512-byte header probe and
selects by descending millisecond mtime, preserving native enumeration ties.
Historical conversion applies only the ID/parent and custom-message passes;
current and future versions remain unchanged. Loading retains raw records, while
selected identity, version and cwd reads can return native invalid-data errors.

```rust
# #[cfg(not(target_arch = "wasm32"))] {
use maestro_session::SessionManager;
use serde_json::json;

let directory = std::env::temp_dir().join(format!("maestro-example-{}", uuid::Uuid::new_v4()));
let directory_text = directory.to_string_lossy();
let mut history = SessionManager::create("work", Some(&directory_text), "unused")?;
let path = history.get_session_file().unwrap().to_owned();
history.append_message(json!({"role":"user", "content":"hello"}))?;
assert!(!std::path::Path::new(&path).exists());
history.append_message(json!({"role":"assistant", "content":"reply"}))?;
let reopened = SessionManager::open(&path, None, None)?;
assert_eq!(reopened.get_entries().len(), 2);
std::fs::remove_dir_all(directory)?;
# }
# Ok::<(), Box<dyn std::error::Error>>(())
```

# Session history and stored-entry context

`maestro-session::session_manager` parses JSONL text you supply and projects a
selected ancestry into context. Native operations create, reopen, switch and
continue JSONL histories, and explicit memory history stays available.
[Message formatting](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/sessions/messages.md)
converts stored timestamps.

The [session format](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/sessions/session_format.md)
describes record shapes, and
[discovery](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/discovery.md)
covers listing sessions.

## Raw entries and explicit views

`FileEntry` and `SessionEntry` are ordered JSON values, not validated schemas.
`parse_session_entries` removes outer ECMAScript whitespace, splits at LF, skips
blank or malformed lines and accepts native JSON values without requiring a
header, including `null`, `false`, zero numbers and empty strings. The native
loader skips those four empty values before it checks the header or builds its
index, and listing skips them too. File loading and listing are unavailable on
`wasm32`; `parse_session_entries` keeps those values on every target.

Nonblank interior lines are parsed without extra trimming. Native JSON numeric and
nesting limits apply. An object member keeps its first position and takes the
value of the last duplicate; omitted and null properties stay distinct.

The named header and entry records implement the standard serde views. Decode from
`&Value` only when you need those fields; raw parsing and projection do not
validate unused payloads. Context messages and settings stay raw, owned values.

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

Without a supplied index, indexing runs before explicit-null selection, and the
last entry with a given ID owns the lookup. A supplied index is authoritative, even
when empty, and may refer to entries outside the slice. An explicit null selects
before the first entry; a missing, empty or unknown leaf ID falls back to the last
supplied entry. A missing parent ends the reachable ancestry. Revisiting an entry
ends the walk before it adds the entry again, so the reachable suffix is kept and
the walk never loops.

Settings are observed across the selected root-to-leaf path before any message is
emitted. Later thinking and model observations overwrite earlier ones. The latest
compaction on the path emits its summary, then its selected precompaction kept
range, then the later messages. Ordinary messages and hidden custom messages
contribute. Custom state, labels, session names and unknown entry kinds do not.
Branch summaries contribute when their raw summary is truthy.

Projection converts the timestamps of generated messages through the existing
[message formatting](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/sessions/messages.md)
owner. Explicit typed message decoding and model conversion stay separate
operations. When projection reaches a null entry or message receiver, it returns a
native error that names the field, not an eager schema error.

`get_latest_compaction_entry` scans backwards through the supplied order, without
ancestry selection, and borrows the matching original element. It accepts owned
entries or borrowed ones, such as a branch. It stops at its first match; a null
entry met before that match is an error.

## In-memory history

`SessionManager::in_memory` creates a memory history that never reads or writes a
session file. An omitted cwd captures the process working directory and propagates
its native lookup error; an explicit string, including an empty one, stays as
authored. Session identities use `UUIDv7` unless `NewSessionOptions.id` supplies a
string. Headers and message entries get fresh UTC millisecond timestamps. An entry
identity tries eight hexadecimal characters from a random UUID, up to 100 times,
before it falls back to a full UUID.

Appending moves the raw message into the retained sequence, indexes it and selects
it as a child of the previous selection. `branch` checks membership before it
selects; a missing target returns exactly `Entry {id} not found`. Abandoned
branches stay in history. `get_branch(None)` uses the selection; an explicit empty
or unknown identity returns an empty path and leaves the selection unchanged.
Context follows the [projection rules](#branch-selection) above.

`ReadonlySessionManager` provides the reads, which borrow retained records (see
[Read-only view](#read-only-view)); import the trait to call them. To keep a record
across mutable operations, take an explicit owned copy.

`new_session` replaces the header, clears entries, index, labels and selection, and
keeps the cwd. It writes no file. Native history selects a fresh filename; memory
history keeps any filename selected explicitly.
`NewSessionOptions.parent_session` preserves missing, null and string metadata.
Memory history reports false persistence and an empty session directory. Ordinary
memory operations do no file I/O; explicit native `set_session_file` may read and
convert records without writing them. `get_entries` borrows every nonheader record
in retained order.

```rust
use maestro_session::session_manager::ReadonlySessionManager;
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

## Appends, names and branch summaries

Each append operation captures the selected entry as parent, a generated identity
and a UTC millisecond timestamp, accepts the record into memory, then runs the
publication routine described under [Native history](#native-history). A
publication error leaves the accepted record and its selection in memory and the file
as it was; the next save writes the record. The operations are `append_message`, `append_thinking_level_change`,
`append_model_change`, `append_compaction`, `append_custom_entry`,
`append_session_info`, `append_custom_message_entry`, `append_label_change` and
`branch_with_summary`.

Optional operands keep omission apart from null: `Presence::Missing` omits the
member and `Presence::Null` writes null. `append_compaction` records the cut
identity you supply without checking that it exists; `CompactionOptions::default()`
omits both optional members. The rules under [Branch selection](#branch-selection)
project custom messages into model context whether or not they are displayed;
custom entries never are. Members are written in a fixed order, one compact
LF-terminated line per record.

`append_session_info` removes outer ECMAScript whitespace (U+FEFF is removed,
U+0085 is kept); an empty result clears the name. `get_session_name` reads the
newest `session_info` record in the whole history, not only the selected branch.
If that newest record's name is missing, null or blank, the result is `None`, and
Maestro never falls back to an older name. If the newest name is present, not null
and not a string, `get_session_name` returns an invalid-data error; older names are
not decoded.

`branch_with_summary(Some(id), ..)` selects an existing entry, and `None` selects
the position before all entries. The supplied summary is appended there with
`fromId` set to the entry identity or the literal `root`. An unknown identity fails
with `Entry {id} not found` before any change. The caller supplies the summary;
nothing is generated.

## Read-only view

`ReadonlySessionManager` is an object-safe trait that `SessionManager` implements
directly: `get_cwd`, `get_session_dir`, `get_session_id`, `get_session_file`,
`get_leaf_id`, `get_leaf_entry`, `get_entry`, `get_label`, `get_branch`,
`get_header`, `get_entries`, `get_tree` and `get_session_name`. It borrows the
manager, so the next borrow sees a later mutation; it is not a snapshot.
[The tree guide](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/tree.md)
describes labels and trees.

## Native history

Native `create` and `continue_recent` use the supplied directory, or encode the
stored cwd beneath the caller's resolved agent root. `open` resolves its explicit
filename lexically and reads it once, line by line. A cwd override wins over the
stored cwd, with a runtime fallback for a missing or null cwd. Explicit directory
spellings are kept. A session directory is a `SessionDir`, which only the session
crate builds, so a caller receives the name instead of spelling one. A report that
reads one keeps that record through its consuming helpers and borrows its text for
filesystem paths and displayed output.

A fresh history publishes at its first assistant message, writing the header and every
record so far in one append. A failed first write is cut back to the file's earlier
length and leaves the history unpublished, so the next save writes it once from the
start. Loaded and recovered histories stay published: each later save appends the
records the file does not hold yet, normally one. A failed later append is cut back to
the file's earlier length too, so no torn line is left for the next record to join, and
the accepted record stays in memory: the next save writes it before the new one, and
every record's parent is in the file. A failed switch keeps the prior manager state. An empty file is initialised immediately at the
selected path. A nonempty file without a valid session header is refused with
`Session file is not a valid maestro session: <path>` and keeps its bytes, so
`open` and `set_session_file` never erase a file that is not a session. A missing
explicit file waits for first publication at that path.

Loading, switching, forking and listing read a session file one line at a time, so
memory use follows the longest line, not the file size; only the records that
loading keeps are held in full. Listing loads the metadata of at most ten session
files at once.

Continuation enumerates names, probes the `.jsonl` candidates, then reads the valid
candidates' metadata in a separate pass. It selects by descending millisecond
mtime; equal times go to the first name in name order. A metadata error discards
the selection; a failed probe skips only that candidate. The header probe reads at
most 512 bytes.

Historical conversion applies only the ID/parent and custom-message passes; current
and future versions stay unchanged. Removing a numeric legacy compaction cut index
preserves the order of the remaining object members. Loading keeps raw records,
while reads of the selected identity, version and cwd can return native
invalid-data errors.

```rust
# #[cfg(not(target_arch = "wasm32"))] {
use maestro_session::session_manager::ReadonlySessionManager;
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

## Branch extraction and forking

`create_branched_session(leaf_id)` replaces the history with the ancestry of
`leaf_id` under a fresh `UUIDv7` header that keeps the owner's cwd. An unknown or
empty identity fails with `Entry {id} not found` before anything changes. Label
records on the ancestry are dropped. A retained record that pointed at one is
reconnected to the nearest ancestor that was not a dropped label, and other records
are copied unchanged. The resolved labels whose targets are retained are recreated
after the retained tail, in resolved order, with their stored timestamps (omitted
when none is stored) and fresh entry identities.

Memory history returns `None` and keeps any selected filename. Native history
returns the new lexical filename and records the previous file as `parentSession`.
It writes the file at once only when a retained message is from the assistant;
otherwise it waits for [first publication](#native-history). Publication overwrites
any bytes already at the new filename, under the shared write lock. A publication
error leaves the new history installed and unsaved: the next save empties the file and
writes the header and every record.

`SessionManager::fork_from(source, target_cwd, session_dir, agent_dir)` copies the
nonheader records of a source file into a new published native history. It reads
the source as `open` does, converts it as [loading](#native-history) converts a
history, and never rewrites it. The new header holds `target_cwd` and the source
path exactly as supplied. An omitted directory is the default for `target_cwd`
beneath `agent_dir`. Once the source is loaded and converted, an empty
`session_dir` path fails with a not-found error before the destination is created.
A source that is empty or has no valid header fails with
`Cannot fork: the source session file has no valid entries: {source}` before anything is
created. A null source record is skipped. The header and every record go to the file in
one append, and a write error leaves whatever bytes that append wrote.

```rust
# #[cfg(not(target_arch = "wasm32"))] {
use maestro_session::session_manager::ReadonlySessionManager;
use maestro_session::SessionManager;
use serde_json::json;

let mut memory = SessionManager::in_memory(Some("work"))?;
let root = memory.append_message(json!({"role":"user", "content":"a", "timestamp":1}))?;
memory.append_message(json!({"role":"user", "content":"abandoned", "timestamp":2}))?;
memory.branch(&root)?;
let leaf = memory.append_message(json!({"role":"user", "content":"b", "timestamp":3}))?;
assert_eq!(memory.create_branched_session(&leaf)?, None);
assert_eq!(memory.get_entries().len(), 2);

let directory = std::env::temp_dir().join(format!("maestro-example-{}", uuid::Uuid::new_v4()));
let directory_text = directory.to_string_lossy();
let mut history = SessionManager::create("work", Some(&directory_text), "unused")?;
history.append_message(json!({"role":"assistant", "content":"reply"}))?;
let source = history.get_session_file().unwrap().to_owned();
let fork = SessionManager::fork_from(&source, "other", Some(&directory_text), "unused")?;
assert_eq!(fork.get_cwd(), "other");
assert_eq!(fork.get_header().unwrap()["parentSession"], source);
std::fs::remove_dir_all(directory)?;
# }
# Ok::<(), Box<dyn std::error::Error>>(())
```

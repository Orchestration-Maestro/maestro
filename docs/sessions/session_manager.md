# Session history and stored-entry context

`maestro-session::session_manager` parses supplied JSONL text and projects a
selected ancestry into context. Native operations create, reopen, switch and
continue JSONL histories; explicit memory history remains available.
Stored timestamp conversion delegates to [message formatting](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/sessions/messages.md).

See also the [session format](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/sessions/session_format.md) for record shapes and
[discovery](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/discovery.md) for listing sessions.

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
selection and borrows the matching original element. It accepts owned entries or
borrowed ones, such as a branch. It stops at its first match;
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

Reads borrow retained records and are provided by `ReadonlySessionManager`
(see [Read-only view](#read-only-view)); import the trait to call them. To keep
a record across mutable operations, callers take an explicit owned copy.
`new_session` replaces the header and clears entries, index, labels and selection while retaining cwd. It writes no file;
native history selects a fresh filename, while memory history retains any
explicitly selected filename.
`NewSessionOptions.parent_session` preserves missing, null and string metadata.
Memory history reports false persistence and an empty session directory. Ordinary
memory operations perform no file I/O; explicit native `set_session_file` may
read and convert records without writing them. `get_entries` borrows every
nonheader record in retained order.

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

Each append operation captures the selected entry as parent, a generated
identity and a UTC millisecond timestamp, accepts the record into memory, then
runs the publication routine described under [Native history](#native-history).
A publication error leaves the accepted record, its selection and any written
bytes in place. Operations: `append_message`,
`append_thinking_level_change`, `append_model_change`, `append_compaction`,
`append_custom_entry`, `append_session_info`, `append_custom_message_entry`,
`append_label_change` and `branch_with_summary`.

Optional operands keep omission apart from null: `Presence::Missing` omits the
member, `Presence::Null` writes null. `append_compaction` records its supplied
cut identity without checking that it exists; `CompactionOptions::default()`
omits both optional members. Custom messages are projected into model context by the
rules under [Branch selection](#branch-selection) whether or not they are
displayed; custom entries never are. Members are written in a fixed
order, one compact LF-terminated line per record.

`append_session_info` removes outer ECMAScript whitespace (U+FEFF is removed,
U+0085 is kept); an empty result clears the name. `get_session_name` reads the
newest `session_info` record in the whole history, not only the selected
branch. A newest record whose name is missing, null or blank gives `None` and
never falls back to an older name. A newest name that is present, not null and not a string
returns an invalid-data error; older names are not decoded.

`branch_with_summary(Some(id), ..)` selects an existing entry, `None` selects the
position before all entries, and the supplied summary is appended there with
`fromId` set to the entry identity or the literal `root`. An unknown identity
fails with `Entry {id} not found` before any change. The summary is supplied by
the caller; nothing is generated.

## Read-only view

`ReadonlySessionManager` is an object-safe trait implemented directly by
`SessionManager`: `get_cwd`, `get_session_dir`, `get_session_id`,
`get_session_file`, `get_leaf_id`, `get_leaf_entry`, `get_entry`, `get_label`,
`get_branch`, `get_header`, `get_entries`, `get_tree` and `get_session_name`.
It borrows the manager, so a later mutation is seen by the next borrow; it is
not a snapshot. Labels and trees are described in [the tree guide](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/tree.md).

## Native history

Native `create` and `continue_recent` use the supplied directory or encode the
stored cwd beneath the caller's resolved agent root. `open` resolves its explicit
filename lexically and reads it once, line by line; cwd override wins over stored cwd, with a
runtime fallback for missing/null cwd. Explicit directory spellings are retained.
A session directory is a `SessionDir`, which only the session crate builds, so a
caller receives the name instead of spelling one. A report that reads one keeps
that record through its consuming helpers and borrows its text for filesystem
paths and displayed output.

A fresh history publishes at its first assistant message. Loaded and recovered
histories stay published: subsequent appends write one record. Append errors leave
accepted memory and any partial bytes in place; failed switches retain the prior
manager state. An empty file is initialised immediately at the selected path. A
nonempty file without a valid session header is refused with `Session file is not a
valid session: <path>` and keeps its bytes, so `open` and `set_session_file` never
erase a file that is not a session. A missing explicit file waits for first
publication at that path.

Loading, switching, forking and listing read a session file one line at a time, so
memory use follows the longest line rather than the file size; the records that
loading keeps are the only part held in full. Listing loads the metadata of at most
ten session files at once.

Continuation enumerates names, probes the `.jsonl` candidates, then reads the
valid candidates' metadata in a separate pass. It selects by descending
millisecond mtime; equal times go to the first name in name order. A metadata
error discards the selection; an individual probe failure skips that candidate.
The header probe reads at most 512 bytes.
Historical conversion applies only the ID/parent and custom-message passes;
current and future versions remain unchanged. Removing a numeric legacy compaction
cut index preserves the remaining object member order. Loading retains raw records, while
selected identity, version and cwd reads can return native invalid-data errors.

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
`leaf_id` under a fresh `UUIDv7` header that keeps the owner's cwd. Unknown and
empty identities fail with `Entry {id} not found` before anything changes. Label
records on the ancestry are dropped; a retained record that pointed at one is
reconnected to the nearest ancestor that was not a dropped label, and other records are copied unchanged.
The resolved labels whose targets are retained are recreated after the retained
tail, in resolved order, with their stored timestamps (omitted when none is stored)
and fresh entry identities.

Memory history returns `None` and keeps any selected filename. Native history
returns the new lexical filename, records the previous file as `parentSession`
and writes the file at once only when a retained message is from the assistant;
otherwise it waits for [first publication](#native-history). A publication error
leaves the new history installed and the previous publication state.

`SessionManager::fork_from(source, target_cwd, session_dir, agent_dir)` copies
the nonheader records of a source file into a new published native history. The
source is read as for `open`, converted as [loading](#native-history) converts a
history, and never rewritten. The new header holds `target_cwd` and the
source path exactly as supplied. An omitted directory is the default for
`target_cwd` beneath `agent_dir`. After the source is loaded and converted, an
empty `session_dir` path fails with a not-found error before the destination is
created. A source that is empty or has no valid header
fails with `Cannot fork: source session file is empty or invalid: {source}`
before anything is created; a null source record and later write errors leave the
records already written.

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

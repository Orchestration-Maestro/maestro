# Stored-entry context

`maestro-session::session_manager` parses supplied JSONL text and projects a
selected ancestry into context without session-file reads or writes.
Generated timestamps delegate to [message formatting](messages.md).

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
[message formatting](messages.md) owner. Explicit typed message decoding and
model conversion remain separate operations. Reached null entry or message
receivers return native errors identifying the field, not an eager schema error.

`get_latest_compaction_entry` scans supplied order backwards without ancestry
selection and borrows the matching original element. It stops at its first match;
a null entry encountered before that match is an error.

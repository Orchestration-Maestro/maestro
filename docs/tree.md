# Labeled history trees

`maestro_session::session_manager` exposes the retained history as a tree and
lets callers label entries. Import `ReadonlySessionManager` for the reads below.
Append rules, names and summaries are described with the
[session history](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/sessions/session_manager.md).

## Tree

`get_tree` returns one `SessionTreeNode` per root, each holding a borrowed
`entry`, owned `children`, and the resolved `label` and `label_timestamp`.
An entry is a root when its parent is null, missing, not a string, itself, or
not present in the history. Roots keep retained order. Each child list is sorted
by stored timestamp, oldest first: equal instants and unparsable timestamps keep
retained order, with unparsable ones after the parsable ones. Timestamps use the
[message formatting](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/sessions/messages.md)
parser, so a date-time without an offset is local time.

When identities repeat, the last record supplies each node's data while every
record contributes its own parent edge, so a repeated identity can appear under
several parents. A walk omits an edge only when it returns to an entry already
on the path being built, so cyclic parents terminate; a cycle unreachable from
any root yields no nodes. Dropping a tree never recurses, so deep histories are
safe.

`get_children(id)` returns the direct children of an identity in the order the
identities first appear, using each identity's last record. It is a method of
`SessionManager`, not of the read-only view, and does not sort.

## Labels

`append_label_change(target, label)` appends a label record below the selected
entry (not below the target) and selects it. A missing, null or empty label
clears the target's label; any other string, including whitespace, sets it. An
unknown target fails with `Entry {id} not found` before any change. The
resolved label and its timestamp change only after the publication routine
(see [native history](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/sessions/session_manager.md#native-history))
returns successfully, so a failed native write leaves the previous labels in
place; a routine that completes without writing still updates them.
Loading a history replays its label records in order and keeps raw values;
`new_session` and history switches replace the resolved labels together with
the entries. Labels never enter model context.

```rust
use maestro_request::presence::Presence;
use maestro_session::SessionManager;
use maestro_session::session_manager::ReadonlySessionManager;
use serde_json::json;

let mut history = SessionManager::in_memory(Some("work"))?;
let first = history.append_message(json!({"role":"user","content":"hello","timestamp":1}))?;
history.append_message(json!({"role":"user","content":"again","timestamp":2}))?;
history.append_label_change(&first, Presence::Present("start"))?;
let tree = history.get_tree();
assert_eq!(tree[0].label, Some(&json!("start")));
drop(tree);
history.append_label_change(&first, Presence::Missing)?;
assert_eq!(history.get_label(&first), None);

history.reset_leaf();
assert!(history.get_leaf_entry().is_none());
let second_root = history.append_custom_entry("state", Presence::Missing)?;
assert_eq!(history.get_tree().len(), 2);
assert!(history.get_entry(&second_root).is_some());
# Ok::<(), Box<dyn std::error::Error>>(())
```

`reset_leaf` selects the position before all entries without writing. The next
append then has no parent and becomes a root, except `branch_with_summary(Some(id), ..)`,
which first selects that existing entry and appends below it.

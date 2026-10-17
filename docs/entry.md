# Session interchange records

`maestro-entry` declares session headers, entry identity, the nine concrete entry
records, new-session options, built-context records and compaction preparation,
results, settings and file-operation records, plus `ForkPosition` and
`NavigateTreeOptions` shared by native session operations and guest authoring. It depends only on `maestro-request` within
the workspace. [Approved libraries](libraries/entry.md) supply serialization and
ordered sets.

## Consumer views

Both consumers retain their existing public names through re-exports or aliases.

| Generic member | Native session view | Extension guest view |
| --- | --- | --- |
| Stored message | Raw JSON value | Typed agent message |
| Entry details or data | JSON value by default; caller-selected type | Opaque JSON text |
| Context setting | `Presence<Value>` | `String` |
| New-session parent | `Presence<String>` | `Option<String>` |
| Preparation message | Caller-selected type | Typed agent message |
| Compaction result details | Caller-selected type, JSON value by default | Opaque JSON text |

Native context views are built by projection. See [session context](sessions/session_manager.md)
for history, file reading and selection, and [guest records](extensions/guest.md#session-entries)
for guest decoding. The entry crate adds no history, discovery, persistence or
compaction algorithm; consumers own those operations.

## Encoded members

Concrete entries write the `type` tag first. Custom and custom-message entries
write their own members before `id`, `parentId`, `timestamp`; the other entries
write those identity members before their own fields. Preparation writes
`firstKeptEntryId`, `messagesToSummarize`, `turnPrefixMessages`, `isSplitTurn`,
`tokensBefore`, optional `previousSummary`, `fileOps`, `settings` in that order.

`CompactionResult<T>` writes `summary`, `firstKeptEntryId`, `tokensBefore`, then
optional `details`. It carries no generation or persistence behavior.

Optional entry/header fields, result details and `previousSummary` use the request owner's
[`Presence<T>`](records.md#stored-session-entries). Detail types need no `Default`
for decoding. [`Omittable`](../crates/maestro-request/src/presence.rs) in the request
owner controls serialization of context settings and the
new-session parent: `Presence::Missing` and `None` are omitted; text is written,
including empty text. These generic fields have no deserialization default.

`FileOperations` uses insertion-ordered sets for `read`, `written`, `edited`.
Serialization writes each set as an array; repeated insertion retains the first
position.

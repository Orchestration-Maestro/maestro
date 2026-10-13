# Session interchange records

`maestro-entry` declares session headers, entry identity, the nine concrete entry
records, new-session options, built-context records and compaction preparation,
settings and file-operation records. It depends only on `maestro-request` within
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

Optional record members use the request owner's
[`Presence<T>`](records.md#stored-session-entries). Detail types need no `Default`
for decoding. `Omittable` controls serialization of context settings and the
new-session parent: `Presence::Missing` and `None` are omitted; text is written,
including empty text. These generic fields have no deserialization default.

`FileOperations` uses insertion-ordered sets for `read`, `written`, `edited`.
Serialization writes each set as an array; repeated insertion retains the first
position.

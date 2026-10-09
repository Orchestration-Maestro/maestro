# Internal response conversion

The response module is crate-internal, with no package-root exports. It compiles
alongside the providers; endpoint adapters will supply its production callers.
Its three operations convert history, convert tool declarations and reduce
already-framed response event text. They open no connection.

## Request items

`convert_responses_messages` uses the existing
[conversation projection](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/conversation-projection.md).
The nonempty system prompt is included by default, as `developer` for reasoning
models and `system` otherwise. User content retains its part order; empty block
lists produce no item, while empty text does.

Assistant text becomes a completed message. Version-one text signatures select
identity and recognized phase; other nonempty signatures supply the identity
literally. Identities longer than 64 UTF-16 units are hashed. Missing or empty
identities use the number of preceding turns that produced items. Thinking is
replayed only when it has a nonempty signature. Replay uses the owning
[JSON conversion rules](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/models/arguments.md);
numeric spelling can change.

Tool identities are normalized for foreign history. The allowed-provider set
selects whether a compound identity retains a separate item identity. Item
identities starting with `fc_` are omitted for a different model only when
provider and API both match. Tool-result images follow the joined text when
supported, including image-only results; otherwise empty text uses
`(see attached image)`. `convert_responses_tools` preserves declaration order
and defaults `strict` to false; an explicit absent strict value sends null.

## Event reduction

`process_responses_stream` selects an event branch before decoding its members.
Unknown events and mismatched deltas leave unused fields unread. Concatenated
values use string rendering, while selected typed assignments reject values the
record cannot represent. Missing or null required containers and malformed
consumed strings return native diagnostics. An initial message's last content part is inspected
only when a matching delta needs its kind.

Text, thinking and tool-call updates share the supplied output handle. Every
opened block retains its index even if a caller appends another block. Final
message items replace provisional text. Reasoning selects nonempty final summary,
then nonempty final content, otherwise retains provisional thinking. An unmatched
final message or reasoning item is ignored. Final text is stored before signature
conversion; a conversion failure emits no end event. Final reasoning retains its
whole item as compact JSON after successful conversion. Encoded message signatures
retain truthy phases; recognized phases are selected only during history conversion.
Existing
calls use nonempty scratch arguments before final arguments and ignore final
identity. Final-only calls are inserted before publishing their end. Argument
completion delegates to the owning JSON helper; nonobject results become `{}`.
Argument completion replaces stored arguments, emitting a delta only when the
completed text adds a nonempty suffix to the previous scratch.

Completion publishes the final nonempty ID and model-priced usage before any
tier callbacks. Resolution runs only with pricing, outside message locks. Priced
usage is published before status selection, including when that selection fails.
Only a stop outcome is upgraded to tool use when a call exists.

Direct errors render `Error Code {code}: {message}`. Failed responses prefer a
truthy error, then a truthy incomplete reason, otherwise no-details text. Objects
render as `[object Object]` without reading their members; arrays are joined
iteratively with commas and empty null entries.

The reducer consumes to source EOF, propagating later failures even after
completion. EOF without a completed or incomplete event returns
`Response stream ended before a terminal event`. It publishes content updates
only; the caller owns final outcome events and stream termination. Public
invocation examples belong to the endpoint adapters.

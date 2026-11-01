# JSON boundary

`maestro-json` is a core leaf below layer 0 with no workspace dependencies. It holds
boundary decisions and synchronous JSONL framing; each caller keeps its own preprocessing,
default values, selection policy and error wording. The [workspace daemon
contract](specs/maestro-port.md#workspace-daemon) also assigns shared JSONL line
framing to this leaf.

- **Truth.** `is_truthy` classifies a `serde_json::Value`; `is_raw_truthy` classifies
  valid raw JSON without reading a container. Null, `false`, zero of either sign and
  the empty string are false; every other string, array and object is true. A number
  beyond the range of a double is an infinity and so true, and one below it is zero and
  so false. Absence and nullish defaults stay with the caller.
- **Selected values.** `display_truthy` returns no text for absent or falsy values,
  borrows nonempty strings unchanged, and owns the compact JSON spelling of other
  truthy values. Callers choose the field and fallback.
- **Keys.** `array_index` accepts ASCII decimal spellings with no sign and no leading
  zero, from `0` to `4294967294`. `canonical_entries` stably places those keys first in
  ascending numeric order and leaves every other entry in the order given; it never drops
  or merges entries.
- **Text.** `compact_json`, `compact_object` and `compact_members` write compact text;
  `pretty_object` writes objects and `pretty_json` accepts any JSON root, both with
  two-space indentation and no final newline. Keys follow
  `canonical_entries` at every depth and every number is the double it names, in
  ECMAScript spelling (`1e+21`, `1e-7`, `0` for negative zero). A nonfinite host float is
  already `null` in a `Value`. Ordinary serde output elsewhere is unchanged.
  `BorrowedJson` lets a caller write a value or an array slice it already holds.
  `compact_members_with_default` prepends a named member before canonical ordering,
  using its first authored value or the caller's fallback, without emitting its
  original occurrences. Other members follow the compact writer's rules.
- **Objects.** The `object` module hands a record only the
  map of a JSON object, so a derived record never reads a positional array. `record_with`
  names the caller's expectation in the error; `record`, `from_str`, `list` and
  `optional_list` use `a JSON object`. A repeated member is handled by the record and the
  deserializer the caller supplies; these readers add no duplicate policy.

The optional-field helper that keeps null apart from absence is `present` in
`maestro-request`; the JSON leaf does not depend on it. Dependencies are listed in
[JSON libraries](libraries/json.md).

## JSONL framing

`jsonl::serialize_json_line` uses the compact writer above and appends one LF.
`jsonl::serialize_native_json_line` retains exact signed and unsigned integers for
native workspace requests; keys and floating-point values use the same canonical
writer. U+2028 and U+2029 remain literal characters inside JSON strings in both modes.

`jsonl::JsonlLineSplitter` accepts byte chunks and returns owned lines, splitting
only on LF and removing one terminal CR. It retains incomplete bytes across
chunks, including split UTF-8 scalars. Completed lines use lossy UTF-8 decoding;
BOM characters remain intact. `finish` emits a nonempty pending byte tail once,
including an empty string when that tail is just CR. A final LF does not create
another empty record. Each push scans only its new bytes, not the retained prefix.

The codec owns no stream, parser, runtime or size limit. Callers supply bytes and
call `finish` at end of input. See [workspace sockets](workspace-daemon.md#sockets).

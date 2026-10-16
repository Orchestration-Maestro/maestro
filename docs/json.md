# JSON boundary

`maestro-json` is a core leaf below layer 0 with no workspace dependencies. It holds
stateless decisions at the edge of JSON data; each caller keeps its own preprocessing,
default values, selection policy and error wording.

- **Truth.** `is_truthy` classifies a `serde_json::Value`; `is_raw_truthy` classifies
  valid raw JSON without reading a container. Null, `false`, zero of either sign and
  the empty string are false; every other string, array and object is true. A number
  beyond the range of a double is an infinity and so true, and one below it is zero and
  so false. Absence and nullish defaults stay with the caller.
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

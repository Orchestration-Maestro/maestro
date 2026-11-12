# Tool arguments

## Streamed arguments

`maestro-models` provides shared parsing and sanitizing operations for
model-supplied JSON.

`maestro_models::parameters::json_parse::parse_json(&str)` parses complete JSON
without repairing string literals. It returns `serde_json::Error` on failure.
Use this strict operation for displayed JSON text; use `parse_json_with_repair`
for completed model arguments and `parse_streaming_json` for incomplete ones.
The browser application facade exposes the strict operation at
`maestro_app::browser::utils::parse_json`.

Strict parsing uses the same binary64 number projection and duplicate-member
resolution as argument parsing. Valid overflow becomes null. Duplicate members
keep their first position and last value, including when that replacement
discards an unrepresentable earlier value. Malformed raw controls remain errors;
strict parsing does not apply the streaming operation's outer Unicode trimming.

`repair_json(&str) -> String` escapes raw controls and invalid backslashes only
inside string literals. Valid escapes and text outside strings stay unchanged.
An unfinished Unicode escape remains invalid. `parse_json_with_repair` tries
strict JSON first, then changed literal repair. Native magnitude and lone-surrogate
errors retry raw projection: overwritten lone-surrogate members may be discarded
when the surviving value is representable. If projection fails, the native error
is retained; failure returns the last attempted strict reader's cause through
`DiagnosticErrorInfo`. See the [whole-value conversion boundary](chat-completions.md#whole-value-conversion)
for unrepresentable member names.

`parse_streaming_json(Option<&str>)` returns a JSON value from the accumulated
prefix. It tries strict original, strict repaired, partial original and partial
repaired input, in that order. The two strict readings run only when the text ends
the way a complete JSON text does: in `}`, `]`, `"`, a digit, `e` or `l`, before
any JSON whitespace. Text that ends otherwise cannot be complete, so the result is
the same without them, and a long argument streamed in small fragments avoids two
full passes over the text received so far per fragment. Missing, blank or unusable
input yields `{}`.
Strict scalar values, including null, remain values; partial root null yields
`{}`. Containers preserve completed members and useful nested prefixes without
inventing values for unfinished keys. Quoted delimiters and completed escapes
survive incomplete strings. If escape decoding fails, recovery truncates at the
last backslash and retries the preceding text.
An incomplete string with a raw newline can leave an empty preview until closed.

Truncated booleans and null complete inside containers. Unfinished decimal or
exponent suffixes retain the numeric prefix. Invalid leading grammar and separators
stop the current member, preserving earlier members.
Numbers round to binary64 doubles: `9007199254740993` becomes `9007199254740992`,
and valid overflow such as `1e400` becomes JSON null, including inside incomplete
containers. Negative zero retains its sign in values; compact JSON writes it as `0`.
Root numbers, including numeric overflow projected to null, require the whole input;
other completed non-null values can precede trailing text. Inner whitespace follows JSON grammar; outer trimming also accepts
Unicode spacing and the byte-order mark, but not U+0085. No prose extraction,
comments, single quotes or unquoted keys are accepted.

Full argument materialization accepts at most 127 nested arrays or objects.
Deeper values fail strict parsing and yield `{}` during streaming, rather than a
truncated container prefix. The bound counts consumed containers, not sibling
width, quoted delimiters or unconsumed trailing text.

`parsed_arguments(&str) -> serde_json::Value` delegates to `parse_streaming_json`
and keeps whatever root the text spells, an object included. It is available at
the crate root, `parameters` and `parameters::json_parse` paths.

`sanitize_surrogates(&[u16]) -> String` preserves BMP text and valid UTF-16 pairs,
removing unmatched surrogate units. Ordinary Rust strings need no cleanup.

```rust
use maestro_models::{parse_streaming_json, parse_json_with_repair};
use serde_json::json;

let preview = parse_streaming_json(Some(r#"{"count":1,"label":"rea"#));
assert_eq!(preview, json!({"count":1,"label":"rea"}));
let completed = parse_json_with_repair("{\"label\":\"ready\n\"}")?;
assert_eq!(completed, json!({"label":"ready\n"}));
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Replay defaults

A provider that replays an earlier call applies its own default to the argument root.
A nullish default replaces only null with an empty object. A truthy default replaces
every falsy root, so null, `false`, zero and the empty string all become one. A
supplied empty array or object is truthy and stays as recorded, and every nonempty
root of any kind is replayed unchanged.

The replay default is one shared operation rather than a rule per provider, so the two
policies differ only in which roots they replace.

History projection keeps the stored call unchanged. Each protocol applies its own
wire default:

| Protocol family | Null | False, zero or empty string | Other roots |
|---|---|---|---|
| Chat and response | JSON text `null` | JSON text of the value | JSON text of the value |
| Cloud conversation | Input field omitted | Unchanged | Unchanged |
| Message and content generation | `{}` | Unchanged | Unchanged |
| Conversation | JSON text `{}` | JSON text `{}` | JSON text of the value |

A null cloud conversation root is a null document, which the request serializer leaves
out, so the replayed tool use carries no input field.
Empty arrays and objects remain arrays and objects in every family. Content
generation also uses `{}` for missing or null incoming function-call arguments;
other supplied roots survive through both direct and cloud entries. Conversation
stream arguments admit encoded JSON text or a dictionary; scalar and array roots
are supplied as encoded text.

Session tool-start and tool-update events carry the original argument value,
including null and arrays. Updates also carry the complete progress result;
progress does not replace the arguments or the retained assistant call.

## Checking against a schema

Argument checking, with conversion and with corrective diagnostics, belongs to
`maestro-schema-validator`; see [Schema validation](schema-validation.md). The models
crate re-exports `validate_tool_call` and `validate_tool_arguments` at its root and
under `parameters`, and the crate itself as `parameters::validation`.

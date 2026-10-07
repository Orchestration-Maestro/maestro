# Conversation projection

`Context` keeps the current `system_prompt`, ordered `messages` and current
`tools` separate. `Message` is exactly user, assistant or tool result. A tool
declaration contains only its name, description and JSON Schema parameters;
execution callbacks and policy belong to the caller. Tool results have no usage
or nested-call field.

`transform_messages(&messages, &model, optional_normalizer)` returns an ordered
model-facing message sequence. Invocation never calls it implicitly. It stores
nothing and does not write supplied history. Prompt and tools remain caller-owned
and are not inputs to this function. Original tool-result details, including
absent, null and object values, remain intact.

## Preservation and omission

Exact requested provider, API and model equality retains text/call signatures,
thinking signatures, signed empty thinking and opaque redacted data. Actual
response model/ID do not redefine that equality. Only nonempty signatures count
as signed: `Some("")` is unsigned, while a space is signed without trimming.
Unsigned blank thinking is omitted. Foreign redacted thinking is dropped;
nonblank foreign thinking becomes unsigned text without trimming. Foreign text
always loses its signature; a foreign call loses only a nonempty thought
signature, retaining `Some("")` even when its identifier changes.

`None` leaves identifiers untouched. A supplied mutable callback runs once per
foreign call in source order, receiving the original assistant and target model.
Identity returns do not update or clear prior mappings. Changed identifiers write
sequential last-write-wins mappings used by later real results, even across
same-model reuse and repair boundaries. Empty normalized IDs change calls but
do not rewrite real results because the result mapping must be nonempty. Earlier
results are never rewritten retrospectively. Collisions are accepted: one real
result satisfies all pending calls with that ID; without it each pending call
receives a synthetic result, including duplicates. Repeated real results remain
in order. Names and completed arguments are preserved.

Unchanged tool calls share their original handles; calls with changed signatures
or identifiers use new handles. Transformation itself does not mutate the input,
but a callback may modify the original shared call, without a lock held during
its invocation. There is no purity or uniqueness requirement. Callback panics
propagate; no recovery output is manufactured. Owned argument snapshots of
signature-stripped calls are retained across callback mutation.

Input capability identifiers are supplied data. Exact `image` retains base64
and MIME strings without fetching, decoding, resizing or re-encoding. Otherwise,
only user block arrays and tool-result blocks are downgraded. User strings and
assistant content are not image-downgraded. Adjacent image runs become one text:

- User: `(image omitted: model does not support images)`.
- Tool result: `(tool image omitted: model does not support images)`.

Supplied text blocks always survive unchanged, including omission literals with
replay metadata and literals after generated placeholders. A preceding matching
literal suppresses a following image placeholder regardless of its metadata.
The other role's literal does not suppress it. Ordinary or empty text separates
runs; repeated downgrade preserves content. Generated placeholders are unsigned.

The complete first pass performs image/replay/identifier conversion, including
failed assistants. The second pass repairs prior pending calls before each
assistant/user boundary and at exhaustion. Error and aborted assistants are then
omitted, but their first-pass mapping effects and real results survive. Successful
assistants install their calls as pending; real-result matching uses the current
normalized IDs as a set. Missing calls receive error results in call order with
`No result provided`, their tool name and absent details. Each insertion samples
integer Unix milliseconds internally; there is no supplied clock or timestamp
and no monotonic-time guarantee. Repair executes no tool and writes no history.

## Controlled example

This offline example needs only the `maestro-models` and `serde_json` crates:

```rust
use maestro_models::{Message, Model, UserContent, UserMessage, transform_messages};

let model: Model = serde_json::from_value(serde_json::json!({
    "id": "fixture", "name": "Fixture", "api": "fixture", "provider": "fixture",
    "baseUrl": "fixture:", "reasoning": false, "input": ["text"],
    "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0},
    "contextWindow": 128000, "maxTokens": 16000
})).unwrap();
let messages = vec![Message::User(UserMessage {
    content: UserContent::Text("hello".into()), timestamp: 1.0,
})];
assert_eq!(transform_messages(&messages, &model, None), messages);
```

## Pure argument validation

`ToolCall.arguments` is a supplied JSON object; constructing a call neither
validates nor authorizes it. `validate_tool_call` selects the first matching
declaration, clones arguments, coerces supported values and validates the whole object.
Validation executes nothing and never mutates declarations or calls. Dispatch
does not automatically validate arguments or invoke execution policy.

The offline checker detects the declared JSON Schema dialect automatically;
format checking follows its documented dialect-dependent behavior. Invalid or
unresolvable schemas, including unsupported dialects, fail with `InvalidSchema`.
In-document references resolve locally. External HTTP/file references cannot
trigger retrieval: dependency retrieval features are disabled and construction
is explicitly offline. The checker has documented limitations for some newer
dialect keywords; this interface does not claim universal keyword coverage.
`UnknownTool` and `InvalidArguments` distinguish the other failures.
The retained `IncompleteArguments` error display remains available, but the
current always-present argument object cannot produce it. Error text never echoes raw names, schemas or arguments.

Supported coercions are deliberately narrower than full schema validation:

- Number/integer: finite nonblank numeric strings (decimal, exponent, hexadecimal,
  octal, binary), booleans to 1/0, null to 0. Fractional values are not rounded to
  integers; nonfinite values are rejected.
- Boolean: exact `true`/`false` strings, numeric 1/0, null to false. String:
  number/boolean scalar text, null to empty. Null: empty string, zero or false.
- Type arrays preserve an already matching type, otherwise trying declaration
  order. No object/string or singleton-array conversions, default insertion or
  extra-property removal occurs.
- Declared object properties, schema-valued additional properties and declared
  homogeneous/tuple array items recurse. Tuple schemas must declare a supporting
  dialect. `allOf` traverses in order; `anyOf`/`oneOf` try independent candidate
  clones in schema order, validating branches with their enclosing dialect and
  local reference scope intact. Whole-object validation still enforces exclusive
  `oneOf`, required fields, bounds, enums and additional-property rules.

Blank-thinking detection and numeric-string edge trimming use exactly
U+0009–U+000D, U+0020, U+00A0, U+1680, U+2000–U+200A, U+2028, U+2029,
U+202F, U+205F, U+3000 and U+FEFF. U+0085, U+180E and U+200B are not
whitespace. Retained thinking text and signatures are never trimmed; whitespace
inside numeric strings and whitespace-only numeric strings remain invalid.

Number-to-string coercion uses binary64 semantics regardless of JSON integer
storage, with shortest round-trip digits. Negative zero becomes `"0"`; integral
values have no fraction. Fixed notation applies from `1e-6` inclusive to `1e21`
exclusive: `1e-6` becomes `"0.000001"`, `1e20` becomes
`"100000000000000000000"`. Outside that range, `1e-7` becomes `"1e-7"`,
`1e21` becomes `"1e+21"` and the smallest positive subnormal becomes `"5e-324"`.
The integer-backed value `9007199254740993` becomes `"9007199254740992"` only
when coerced to a string; a value already accepted by a numeric schema stays
unchanged. String enums are checked against the coerced spelling.

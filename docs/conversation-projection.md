# Conversation projection

`Context` keeps the current `system_prompt`, ordered `messages` and current
`tools` separate. `Message` is exactly user, assistant or tool result. A tool
declaration contains only its name, description and JSON Schema parameters;
execution callbacks and policy belong to the caller. Tool results have no usage
or nested-call field.

`project_context` explicitly constructs a pure, deterministic request view.
Invocation never calls it implicitly. It stores nothing and never edits supplied
prompt, tools, history or image strings. Tool-result `details` are removed only
from this explicit projected view; invocation itself forwards them unchanged.

## Preservation and omission

Exact requested provider, API and model equality retains text/call signatures, thinking signatures, signed empty thinking and opaque redacted data.
Only nonempty signatures count as signed: `Some("")` is unsigned, while a
signature containing a space is signed without trimming.
Actual response model/ID do not redefine that equality. Unsigned blank thinking
is omitted. Foreign replay drops text/call metadata and redacted thinking;
nonblank readable thinking becomes unsigned text, without exposing opaque data.
Supplied shared event handles observe later signature updates.

Only foreign calls invoke the supplied deterministic, effect-free
`normalize_tool_call_id` rule. An identity rule changes nothing. Other rules
must preserve distinct IDs within a batch. The originating-turn mapping also
rewrites real and synthetic results; a later same-model turn reusing an ID is
not affected by an earlier rewrite. Mappings survive synthetic repair and
intervening turns, so a delayed real result keeps the same rewritten ID as its
originating call. Names and completed arguments stay exact.

Input capability identifiers are supplied data. `image` retains exact base64
and MIME strings, without fetching, decoding, resizing or re-encoding. Otherwise,
adjacent image runs become one text block:

- User: `(image omitted: model does not support images)`.
- Tool result: `(tool image omitted: model does not support images)`.

Supplied text blocks always survive unchanged, even adjacent omission-literal
blocks with distinct replay metadata or supplied literal text following a
generated omission. A preceding supplied omission literal suppresses a new
placeholder for the following image run regardless of its replay metadata.
Ordinary or empty text separates runs; repeated projection preserves the output. At each next user/assistant boundary and transcript end,
unanswered successful calls receive error results in call order: `No result
provided`, no details, and the supplied synthetic timestamp. Existing matching
results prevent duplicates. Error and aborted assistant attempts are
omitted, without synthesizing their results or inventing completed arguments.
Repair is not tool execution, a model retry or a history write.

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

# Schema validation

`maestro-schema-validator` checks values against a JSON Schema and converts
model-supplied tool arguments before they reach a tool. `maestro-models` re-exports
it as `maestro_models::parameters::validation`, and re-exports
`validate_tool_call` and `validate_tool_arguments` at its root.

## Quick start

Declare a tool, then check an invocation against it:

```rust
use maestro_interchange::types::{Tool, ToolCall};
use maestro_schema_validator::validate_tool_arguments;
use serde_json::json;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let tool: Tool = Tool {
    name: "count".into(),
    description: "Count supplied items".into(),
    parameters: json!({
        "type": "object",
        "properties": {"count": {"type": "integer", "minimum": 1}},
        "required": ["count"]
    })
    .into(),
};
let mut call = ToolCall {
    id: "call-1".into(),
    name: "count".into(),
    arguments: json!({"count": "42"}),
    thought_signature: None,
};
let checked = validate_tool_arguments(&tool, &call)?;
assert_eq!(checked["count"].as_f64(), Some(42.0));
assert_eq!(call.arguments["count"], "42");

call.arguments = json!({});
let error = validate_tool_arguments(&tool, &call).unwrap_err();
assert_eq!(error.message,
    "Validation failed for tool \"count\":\n  - count: must have required properties count\n\nReceived arguments:\n{}");
# Ok(())
# }
```

`validate_tool_arguments` returns the converted argument value. The call and the
declaration stay unchanged. A failure returns a `DiagnosticErrorInfo` whose message lists
at most eight field diagnostics and then the original arguments.

## Checking without conversion

`validate_schema` checks an original JSON value without converting it or changing the
schema. It returns diagnostic lines for invalid values, an empty list for admitted
values, or `DiagnosticErrorInfo` when schema preparation fails. It uses the checker
described under [invocation checking](#reference-invocation-checking), without the
invocation envelope or received-arguments rendering.

## Reference: invocation checking

`validate_tool_call` selects the first declaration with the invocation's exact
name. `validate_tool_arguments` checks one supplied declaration independently of
its name. Both return the owned validated argument value, whatever its root kind,
and leave the invocation and schema unchanged.

A declaration whose schema was built by a schema builder carries the kind of each node
its author declared, beside the ordinary JSON Schema value. Preparation converts those
authored nodes first, as the builder declared them, and then runs the same
primitive conversion over the plain schema. A schema supplied as ordinary JSON carries
no declared kinds, so only the primitive conversion applies: `null` for a required
string member stays `""` there, while the same member of an authored schema becomes
`"null"`. The authored pass keeps in-place container edits but discards a returned
root replacement. Tuple conversion assigns positions in place; array conversion
returns a mapped copy while retaining nested container edits. Schema text never
implies author intent; rebuilding it needs the builder.
A cyclic node's definitions are visible only while its reference converts, so a
sibling member sees the enclosing definitions again. A reference names the innermost
definition visible at the point where conversion reaches it. Union alternatives are
checked against those same definitions, and a composed alternative keeps the constraints
and optional members its author declared. An authored union checks the original value
first. If no alternative accepts it, every alternative converts an independent candidate
before selection begins; a later conversion error is not hidden by an earlier match.

Two authored nodes are compared by what each kind accepts. An alternative set never
stands in for a sequence or an intersection of the same members, so its alternatives keep
their own conversions. A member an object leaves optional never satisfies a member the
compared object requires. Each object reads its required names once when its members are
projected, so construction work grows with the member count rather than with its square.
An authored kind is looked up by the identity of the schema node it describes, resolved
once from the pointer the builder recorded.

Checking converts declared primitive values before evaluating the original schema.
Objects and arrays are traversed only when their type is explicitly declared.
Combinator alternatives use independent candidates; the first valid converted
alternative wins, including edits made by its custom checks. Rejected candidates lose
their argument edits, not external callback effects. Type unions preserve values that
already match a member. Missing values are not defaulted and unknown fields are not removed.

A `ParameterSchema<SchemaCode>` retains local callbacks separately from its JSON.
An authored Base node runs its supplied converter during the authored pass. Nested
replacements are kept; root replacements are discarded, while in-place edits remain.
Conversion and acceptance propagate callback errors unchanged, without requesting
error text for successful union selection.
The checker runs a supplied Base guard before literals and compositions, even on a
node without a declared Base kind. Refinements run after that node's ordinary
assertions succeed. Checking stops at a failed refinement; diagnostic collection
visits each refinement and requests error text only for failures. Equal messages
from separate refinements remain separate lines. At most eight lines are retained,
but later error factories still run and their errors propagate unchanged.
Ordinary schema JSON never reconstructs these callbacks.
Compiled checking still checks a required property after a dependency callback
removes it. Optional properties and interpreted candidate checking skip missing
members. Author callbacks accept JSON values, so a deleted member does not invoke
its own Base guard or refinement.

The checker supports object, array, string, number and composition constraints,
offline schema resources, anchors, recursive and dynamic references, and evaluated
property/item tracking. Both tuple forms can occur in one schema. References are
never fetched; unknown resources and non-progressing cycles reject normally.
Every string `$id`, including a fragment-only identifier, starts a resource that
is resolved once against its enclosing resource. Relative `$ref` and
`$dynamicRef` targets resolve against the resource that contains the reference,
not the document root, however that resource was reached: by identifier,
pointer, anchor or dynamic binding. A `$recursiveRef` searches only its own
resource; when that resource declares `$recursiveAnchor: true` it searches only
the subtree of the first entered schema declaring one and resolves against the
resource containing that schema. Entering any schema, with or without `$id`, binds
its own `$recursiveAnchor: true` and `$dynamicAnchor`; entering a schema with `$id`
also binds the dynamic anchors declared below it, arrays of schemas included,
without crossing a nested `$id`. Following a reference keeps the caller's live
anchors, enters the target's enclosing resources that are not yet entered, then
enters the target. The outermost binding of a name wins, a sibling property
never sees bindings added for another, and a pointer fragment never takes a
dynamic binding. A pointer fragment resolves at the resource root first; when
that root does not hold it, the child subtrees answer in declaration order and
the last one that declares the pointer wins. Array pointer segments use exact
unsigned index names: `0` and `1` resolve, while `+0`, `+1` and `01` do not.
Known string formats are asserted, unknown format names remain annotations, and
string lengths count extended grapheme clusters.

Declared conversions run first, skipping a combinator alternative whose own
expressions do not compile. Then, before any assertion is evaluated, the
checker builds once the Unicode expression of every `pattern` and every
`patternProperties` key in the schemas it can reach: the nested schemas of each
keyword and the targets of `$ref`, `$recursiveRef` and `$dynamicRef`, whatever
the arguments are. A target is built in every scope that reaches it, because a
dynamic reference selects its target from the anchors live in that scope.
Unreferenced `$defs` and `definitions`, `then` and `else` without `if`, data such
as `const` and `enum`, and keyword values that are not schemas are never built.
`properties`, `patternProperties`, `dependencies`, `dependentSchemas` and
`dependentRequired` are read only when they hold an object; a list in their place
is ignored. An expression that does not compile is a schema error, not a failed
assertion: the call returns the expression error alone, so no union tolerates it
and no negation inverts it. When several are malformed, the one built first is
reported: object and array keywords with their nested schemas, then the
`pattern` beside them, then reference targets, then conditionals, negation and
combinators.

Evaluated properties and items come only from schemas that succeed. A failing
branch, a negation, a failing `then` or `else`, and the items checked by `contains`
cover nothing, and the properties a nested value evaluated never cover those of
its parent.

Failures report at most eight field diagnostics in evaluation order,
followed by the original arguments as two-space-indented JSON. Repeated identical
ordinary messages, including missing-dependency messages, appear only once;
separate refinements may retain equal messages. Numeric-index
keys precede ordinary insertion-ordered keys. Returned diagnostics have an error
name and message, without a manufactured stack or code. A supplied callback error
instead retains its original fields.

The agent's local tool records retain schema callbacks through preparation and typed
execution. Model requests use `Tool::data()` to copy the schema without its executable
annotations; projecting a request never calls those annotations.

# Content-generation conversion

`maestro_models::providers::content::google_shared` converts a conversation, its tools
and finish reasons into the serializable content turns of the Gemini-style protocols.
It sends nothing; requests and streams are built on top of it, and
`process_google_stream` reduces the responses they receive. `GoogleThinkingLevel`
is also exported at the crate root.

`convert_messages(model, context)` first runs the [conversation projection](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/conversation-projection.md)
for `model`, which decides replay identity, image placeholders, failed turns and
missing tool results, then emits user and model turns. Blank assistant text and
thinking is omitted (script whitespace, including the byte-order mark, counts as blank); an empty user block
list or an assistant message left without parts produces no turn. A thought
signature survives only on content the projection keeps from the same provider, API and
model, and only when it is nonempty, a multiple of four long, made of base64 characters
and ends in at most two `=`; it is not decoded. Call identifiers are emitted only for `claude-` and `gpt-oss-`
models, where a replay across models also rewrites them.

Tool results become `functionResponse` parts under `output`, or under `error` when the
result failed. Text blocks are joined with newlines; a result whose retained content is only
images answers `(see attached image)` (images the model cannot accept are replaced by the
projection first). A result joins the previous user turn only when that turn is the last
emitted turn and already holds function responses. Retained images nest in the response
from Gemini 3 on and for every other family; older Gemini models receive them in a
following user turn that starts with `Tool result image:`, which the next result does not join.

`convert_tools(tools, use_parameters)` returns `None` for no tools. With `false` each
schema is sent whole as `parametersJsonSchema`; with `true` it is sent as `parameters`
with `$schema`, `$id`, `$anchor`, `$dynamicAnchor`, `$vocabulary`, `$comment`, `$defs`
and `definitions` removed from every schema node, including schemas inside `properties`
maps and `items`, `anyOf`, `oneOf`, `allOf` and the other schema-holding keywords. Property
names and literal values (`default`, `const`, `enum`, `examples`) stay as written, as do
`$ref` and all other members.

`map_tool_choice` answers `AUTO`, `NONE` or `ANY`, and `AUTO` for any other string.
`map_stop_reason` accepts the 17 declared finish strings (`STOP` is `stop`,
`MAX_TOKENS` is `length`, the rest `error`) and fails on any other string with
`Unhandled stop reason: <reason>`; `map_stop_reason_string` never fails and answers `error`
for every string except those two. `is_thinking_part` looks only at the thought flag, and
`retain_thought_signature` replaces the held signature with a nonempty incoming one and
otherwise keeps the held one, even when it is empty.

```rust
use maestro_models::Tool;
use maestro_models::providers::content::google_shared::convert_tools;
use serde_json::json;

let tools = vec![Tool {
    name: "inspect".to_owned(),
    description: "Use inspect".to_owned(),
    parameters: json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "urn:a",
        "$anchor": "a",
        "$dynamicAnchor": "dyn",
        "$vocabulary": {"vocab": true},
        "$comment": "note",
        "$defs": {"a": {"type": "string"}},
        "definitions": {"b": {"type": "number"}},
        "type": "object",
        "properties": {"value": {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "urn:a",
            "$anchor": "a",
            "$dynamicAnchor": "dyn",
            "$vocabulary": {"vocab": true},
            "$comment": "note",
            "$defs": {"a": {"type": "string"}},
            "definitions": {"b": {"type": "number"}},
            "type": "string",
            "$ref": "#/$defs/a",
            "$dynamicRef": "#a",
            "pattern": "^keep$"
        }},
        "required": ["value"],
        "additionalProperties": false,
        "default": null
    })
    .into(),
}];
let declarations = convert_tools(&tools, true).expect("one tool is declared");
assert_eq!(
    serde_json::to_value(declarations).expect("declarations serialize"),
    json!([{"functionDeclarations": [{
        "name": "inspect",
        "description": "Use inspect",
        "parameters": {
            "type": "object",
            "properties": {"value": {
                "type": "string",
                "$ref": "#/$defs/a",
                "$dynamicRef": "#a",
                "pattern": "^keep$"
            }},
            "required": ["value"],
            "additionalProperties": false,
            "default": null
        },
    }]}]),
);
```

## Response reduction

`process_google_stream(chunks, output, events, model, counter)` reads framed response
items, one JSON text each, into the shared assistant message `output` and publishes
text, thinking and tool-call events on `events`. It publishes no start, done or error
event and does not end the stream: the caller owns the initial message, those lifecycle
events, cancellation and the stream's end. It adds no HTTP, framing, retry or timeout.

Only the first candidate of an item contributes. The first nonempty response identifier
stays; an empty or absent one is replaced by a later item's. A part whose `thought` flag is
`true` is thinking and every other part with text is text; a signature never decides.
Consecutive parts of one kind extend the open block, a part of the other kind or a function
call closes it first, and a part with both text and a function call applies the text first.
A nonempty signature replaces the block's held one. Image, code and signature-only parts
change nothing.

A function call becomes one complete tool call: a start, one delta holding its arguments as
[compact JSON](arguments.md#streamed-arguments), and an end. Missing or null arguments are
an empty object. The service's identifier is kept when it is nonempty and no call in the
message has it; otherwise the call gets `<name>_<epoch milliseconds>_<n>`, where `<n>`
is the next value of the caller's `counter` (a missing name reads `undefined`). A call
signature is kept when nonempty.

A nonempty finish reason is mapped with `map_stop_reason`, and any call in the message
then turns a `stop` outcome into `toolUse`; `length` and `error` outcomes stay, and a reason
that maps to `error` is recorded, not returned.
A usage report replaces the previous one: input is the prompt count minus the cached count
(without clamping), output is the candidate plus thought counts, cache read is the cached
count, cache write is zero, and the total is the reported total, never a sum. Cost is
recalculated from the model's rates. Absent and null counts are zero, and an absent or null
report keeps the previous one.

A clean end of `chunks` closes the open text or thinking block. A failure from the
source (returned unchanged and never polled past), malformed JSON, a failed typed read of
a selected member, call arguments beyond the JSON conversion's nesting bound, or an
undeclared finish reason returns without closing it, leaving earlier updates in `output`.
A call closes the open block and takes its identifier before its arguments are converted,
so an argument failure keeps those earlier effects. A part's text, thought flag and
signature are read together; unknown members and later candidates are never decoded.

## Request preparation

`build_params` in `maestro_models::providers::content::google` prepares the parameters of a
content-generation request. It sends nothing, calls no hook and never reads a retained timeout,
retry or transport setting; `GoogleOptions` is also exported at the crate root. Two operations compose:

- `build_simple_options(model, options)` selects raw options from simple ones.
- `build_params(model, context, options)` builds `GenerateContentParameters` from the
  [converted](#content-generation-conversion) context and raw options.

```rust
use maestro_models::providers::content::google::{build_params, build_simple_options};
use maestro_models::{Context, Model, SimpleStreamOptions, StreamOptions, ThinkingLevel};
use serde_json::json;

let model: Model = serde_json::from_value(json!({
    "id": "gemini-2.5-flash", "name": "Gemini", "api": "google-generative-ai",
    "provider": "google", "baseUrl": "https://example.invalid/v1beta", "reasoning": true,
    "input": ["text"], "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0},
    "contextWindow": 1_000_000, "maxTokens": 8192
})).expect("model");
let context: Context = serde_json::from_value(json!({
    "messages": [{"role": "user", "content": "hello", "timestamp": 1}]
})).expect("context");
let simple = SimpleStreamOptions {
    common: StreamOptions { api_key: Some("fixture-key".to_owned()), ..StreamOptions::default() },
    reasoning: Some(ThinkingLevel::Low),
    ..SimpleStreamOptions::default()
};
let options = build_simple_options(&model, Some(&simple)).expect("a key is supplied");
let params = build_params(&model, &context, Some(&options)).expect("not aborted");
let wire = serde_json::to_value(&params).expect("serializes");
assert_eq!(
    maestro_models::compact_json(&wire["config"]["thinkingConfig"]).expect("compact"),
    r#"{"includeThoughts":true,"thinkingBudget":2048}"#,
);
```

On success `build_params` returns the model identifier, the converted turns and a configuration.
Temperature and output limit are kept as supplied, zero and negative values included; the
system prompt is omitted only when empty and is otherwise kept untrimmed. Tools are
declared whole (`parametersJsonSchema`), and a tool mode is emitted only when the context
declares at least one tool. A signal supplied already aborted fails with `Request aborted`;
an active one is retained and serializes as `{}`, so aborting the original later is seen
through the prepared value.

Raw thinking is never inferred. Without a thinking request, or for a model that does not
reason, no thinking configuration is emitted. An enabled request on a reasoning model
includes thoughts and carries its level, or its budget when no level is given; budgets are
not clamped. A disabled request ignores any level and budget and selects the family's
lowest setting without thoughts: `LOW` for third-generation Pro, `MINIMAL` for
third-generation Flash and fourth-generation Gemma (recognized case-insensitively
anywhere in the model identifier, with ASCII digits only), and a zero budget otherwise.

`build_simple_options` fails with `No API key for provider: <provider>` unless a nonempty
supplied key (kept untrimmed) or the provider's environment key exists; it does so before
anything else and does not inspect the context or the signal, which `build_params` checks
later. Common settings come from [`build_base_options`](../model-options.md), which also decides the
output limit; no thinking tokens are added;
the simple tool choice is not forwarded. Missing reasoning selects disabled thinking. A
requested effort is first clamped to the model's supported levels, and off and extra-high
both select high. Level-based families (Pro and Flash of the third generation, Gemma 4)
map the effort to a level and ignore custom budgets; other models use the selected
`thinking_budgets` slot, an explicit zero included, or the family default: Pro
`128/2048/8192/32768`, Flash-Lite `512/2048/8192/24576`, Flash `128/2048/8192/24576`
(minimal to high; `2.5` identifiers, matched case-sensitively) and `-1` (dynamic) otherwise.

## Request projection

`maestro_models::providers::content::google_shared::project_content_request` takes the JSON a
payload hook returned and selects the model path and wire body of the public endpoint. It calls
no hook, sends nothing and builds no URL or header: the endpoint, API version, HTTP options and
cancellation stay with the transport caller, and a client-only member is never a body member.

```rust
use maestro_models::compact_json;
use maestro_models::providers::content::google_shared::project_content_request;
use serde_json::{Value, json};

let projected = project_content_request(json!({
    "model": "gemini-fixture",
    "contents": "hello",
    "config": {"temperature": 0, "serviceTier": "flex"},
})).expect("supported members");
assert_eq!(projected.model, "models/gemini-fixture");
assert_eq!(
    compact_json(&Value::Object(projected.body)).expect("compact"),
    r#"{"contents":[{"parts":[{"text":"hello"}],"role":"user"}],"serviceTier":"flex","generationConfig":{"temperature":0}}"#,
);
```

The result holds the model resource path (`models/` is added unless the name starts with
`models/` or `tunedModels/`) and the body, whose key order and number spelling are written by
the [JSON owner](../json.md). A model that is missing, empty or not text, or that contains
`..`, `?` or `&`, is rejected before contents and configuration are read; the first rejection
in the order model, contents, configuration wins.

- Contents accept a content, a text, an admitted part, or a non-empty homogeneous list of
  contents or admitted parts. Empty lists, mixed content/part lists, and bare function-call
  or function-response parts (single or in a list) are rejected.
- A part may hold several members. File data, inline data and function calls select only
  the members the endpoint knows; other supported part members are copied whole.
- Selected record members omit missing or null values, retain zero and false, and apply
  field-specific transforms to empty text, lists and objects. Unsupported record members
  are rejected on presence, even when null or false.
- Sampling and output settings go into `generationConfig`; the system instruction, safety
  settings, tools, tool configuration, cached content and service tier go to the body.
- A schema is converted node by node: type names are uppercased (unknown names become
  `TYPE_UNSPECIFIED`), type arrays and `anyOf` lists drop their null type into `nullable`,
  `items`, `anyOf` and `properties` are converted recursively, and `additionalProperties`
  is dropped at converted nodes. A union collapses to its other arm only when it is a pure
  nullable wrapper; a parent that holds other non-null members is retained for ordinary
  schema conversion. At the responseSchema and function parameters/response fields only,
  a schema carrying `$schema` is moved to its JSON-schema alternative when that destination
  is absent, null, false, zero or empty text, and is not converted in that alternative.


### Cloud projection

`google_shared::project_vertex_content_request(params, project, location)` projects the
selected cloud payload using the same content grouping and [schema/JSON owners](#request-projection).
It reads no environment, calls no hook, authenticates nothing and sends nothing.
The [cloud entry](#cloud-invocation) uses this projector and the existing
[native credentials](#native-credentials).

Cloud model resources retain `projects/`, `publishers/` and `models/`; other names gain
`publishers/google/models/`, or use the first two segments as publisher and model.
Supplied project/location qualify cache names without trimming; absent scope spells
`undefined` in those resource positions. An absent or null cloud model is diagnosed
only after contents and configuration, while an invalid present model fails first.

Cloud parts keep file data, inline data and function-call records whole, but reject
`toolCall`, `toolResponse` and `partMetadata` on presence. Cloud tools keep nested
records whole, select six declaration fields after shared schema preparation, and
reject `fileSearch`, `mcpServers` and declaration `behavior` on presence.
Cloud configuration retains routing, model selection (as `modelConfig`), audio timestamps,
whole safety records, body-level labels and model armor. It rejects
`enableEnhancedCivicAnswers` and tool configuration's `includeServerSideToolInvocations`
on presence; function-calling configuration stays whole, including streamed arguments.
Image output aliases form `imageOutputOptions`; an explicit object shallowly overlays
its members after the aliases. The public endpoint's policies remain unchanged.

```rust
use maestro_models::providers::content::google_shared::project_vertex_content_request;
use serde_json::json;

let projected = project_vertex_content_request(json!({
    "model": "acme/model-a/ignored", "contents": "hello",
    "config": {"cachedContent": "entry", "modelSelectionConfig": {"preference": "quality"}}
}), Some("sample-project"), Some("sample-region")).expect("supported payload");
assert_eq!(projected.model, "publishers/acme/models/model-a");
assert_eq!(projected.body["cachedContent"],
    "projects/sample-project/locations/sample-region/cachedContents/entry");
assert_eq!(projected.body["generationConfig"]["modelConfig"]["preference"], "quality");
```


## Request transport

[`create_client`](https://github.com/Orchestration-Maestro/maestro/blob/main/crates/maestro-models/src/providers/content/google.rs)
captures the supplied endpoint, key and headers without environment lookup or validation.
[`ApiClient::prepare_request`](https://github.com/Orchestration-Maestro/maestro/blob/main/crates/maestro-models/src/providers/content/google_shared/api_client.rs)
consumes a projected body and borrows the selected HTTP options, returning a POST request
and positive timeout milliseconds without sending or starting a timer. Cancellation is left
unset for the invocation caller.

Header layers replace exact spellings before case-insensitive combination. Extra body values
merge recursively only for object/object pairs; other values replace the previous value.
Body text is written by the [JSON owner](../json.md). This stage adds no SDK telemetry headers
and emits no console diagnostics.

```rust
use maestro_models::Model;
use maestro_models::providers::content::google::create_client;
use maestro_models::providers::content::google_shared::project_content_request;
use serde_json::{Value, json};

let model: Model = serde_json::from_value(json!({
    "id": "gemini-fixture", "name": "Gemini", "api": "google-generative-ai",
    "provider": "google", "baseUrl": "https://example.invalid/v1beta", "reasoning": false,
    "input": ["text"], "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0},
    "contextWindow": 8192, "maxTokens": 4096
})).expect("model");
let client = create_client(&model, "fixture-key", None, None);
let mut selected = json!({
    "model": "gemini-fixture", "contents": "hello",
    "config": {"httpOptions": {"timeout": 1001}}
});
let controls = selected.get_mut("config").and_then(Value::as_object_mut)
    .and_then(|config| config.shift_remove("httpOptions"));
let projected = project_content_request(selected).expect("supported payload");
let runtime = tokio::runtime::Builder::new_current_thread().build().expect("runtime");
let prepared = runtime
    .block_on(client.prepare_request(projected, controls.as_ref()))
    .expect("valid transport");
assert_eq!(prepared.request.url,
    "https://example.invalid/v1beta/models/gemini-fixture:streamGenerateContent?alt=sse");
assert_eq!(prepared.timeout, Some(1001.0));
assert!(prepared.request.signal.is_none());
```


## Public invocation

Assistant output uses `google-generative-ai` even when the supplied model names another
API; request processing and payload hooks still receive the original descriptor.

[`stream_google`](../../crates/maestro-models/src/providers/content/google.rs) takes
`GoogleOptions` and returns an independently produced event stream. Default native callers need a
running Tokio runtime. Dropping the event reader does not cancel the producer; its retained
result still observes completion. The module's `stream_simple_google` first selects
[simple options](#request-preparation) and returns a missing-key error before creating a
stream. The [root simple wrapper](module-linkage.md#root-entry-points) settles that error
into a stream.

`stream_google_for` and `stream_simple_google_for` accept an explicit execution
family and producer submitter, with `GoogleOptions<E>` retaining that family's
hooks. Content generation invokes the payload hook, not the response hook.
The public preparation helpers retain their default signatures and do not invoke hooks.

On native targets, constructor settings are captured before the payload hook. An empty
model endpoint uses the matching trimmed environment endpoint: `GOOGLE_GEMINI_BASE_URL`
for public mode or `GOOGLE_VERTEX_BASE_URL` for cloud mode. A present
`GOOGLE_GENAI_USE_ENTERPRISE` flag takes precedence over `GOOGLE_GENAI_USE_VERTEXAI`;
only trimmed, case-insensitive `true` enables cloud mode. Browser calls read no environment.
Explicit public model endpoints remain untrimmed and suppress the appended API version;
environment endpoints keep `v1beta` in public mode or `v1beta1` in cloud mode.
The borrowed `create_client` helper itself still reads no environment.

Ambient cloud mode keeps authentication separate from resource routing. Public raw key
resolution retains a present empty key; it does not automatically select native credentials.
Ambient project/location together with an implicit SDK environment key can select native
authentication instead. The public simple entry keeps its missing-provider-key error.
Public raw browser calls accept API-key authentication from model or hook-selected
`x-goog-api-key` headers even with an empty constructor key. An empty key without
that effective header fails after the payload hook.

After [parameter preparation](#request-preparation), the optional payload hook is awaited
once and its selected payload passes through [projection](#request-projection) and
[transport preparation](#request-transport). Dispatch makes one attempt through
`StreamOptions::fetch` or the default transport. Common timeout/retry settings, transport
selection and the response hook are not consumed. Only a positive selected
`config.httpOptions.timeout` sets a deadline, spanning native authentication, dispatch, error-body admission and
reduction. Native credential acquisition is allowed to finish, but does not reset an
expired deadline. The hook's `PayloadHookInput::signal` starts as the caller's signal and may be
replaced or set to `None`. The returned handle controls request cancellation and is checked
before dispatch; the caller's original signal still determines final abort classification.
Timeout cancellation belongs to the request and does not abort the caller's signal.

A successful status publishes `start` before body reduction; bodyless 204/205 fail afterward,
while an empty streamed 200 completes normally. Failure statuses consume their complete
body before `start`: a case-sensitive `application/json` substring selects whole JSON,
otherwise text is wrapped with status and reason phrase. Body/JSON failures win at admission.

The [shared HTTP framing owner](../../crates/maestro-models/src/providers/http/server_sent_events.rs)
selects whole data segments for this protocol; other callers retain field framing. Ordered
items feed the existing [response reducer](#response-reduction); a JSON null root becomes
an empty response before reduction. A non-whitespace EOF tail
fails. Complete non-data segments and the residual tail share bare-error classification:
for bodies accepted by the JSON decoder within its nesting bound, a numeric or decimal
numeric-string code in `[400, 600)` produces a provider error across chunk and delimiter
boundaries; other complete non-data segments are ignored.
Source and framing failures preserve
prior content without closing its active block; [reducer failures](#response-reduction)
retain the effects preceding their failing conversion. Clean EOF uses the shared conclusion policy.

## Cloud invocation

Assistant output uses `google-vertex` even when the supplied model names another API;
request processing and payload hooks still receive the original descriptor.

[`google_vertex`](../../crates/maestro-models/src/providers/content/google_vertex/mod.rs)
exports `GoogleVertexOptions`, `stream_google_vertex`, `stream_simple_google_vertex`
and their `_for` variants, also available at the crate root. Raw options add optional
`project` and `location` to common controls, tool choice and the existing `GoogleThinking`.
Simple options forward common controls, not raw scope fields. Setup failures settle into
the returned stream. Both entries reuse the [owned invocation](#public-invocation),
[projection](#cloud-projection), [transport](#request-transport) and [credential](#native-credentials)
owners; neither invokes the response hook. Generated tool identifiers use a separate
cloud counter.

The first nonempty trimmed explicit key or `GOOGLE_CLOUD_API_KEY` is selected before
rejecting `gcp-vertex-credentials` and nonempty angle-bracket placeholders. Without a real
key, raw project falls back to `GOOGLE_CLOUD_PROJECT`, then `GCLOUD_PROJECT`; location
falls back to `GOOGLE_CLOUD_LOCATION`. These scope values are not trimmed. Missing project
or location fails before the payload hook. Native credential-file discovery remains later,
at request preparation, so the file may first appear in the hook.

Explicit cloud requests use `v1`. Empty, whitespace-only and literal `{location}` template
endpoints are ignored. Other explicit endpoints are trimmed and select collection scope;
a complete lowercase ASCII `v<digits>` or `v<digits>beta<digits>` pathname segment suppresses
the added version (digits after `beta` may be absent). Default native hosts distinguish
global, `us`/`eu` and regional locations. Environment-selected hosts retain native scope.
Hook HTTP controls may override endpoint, version and resource scope; only exact
`COLLECTION` suppresses the native project/location prefix. Nonempty routing keys and
already-qualified `projects/` model paths also suppress that prefix. Authentication remains
independent of those resource decisions.

Cloud thinking differs from public thinking: Gemma uses budgets, Flash-Lite's minimal
budget is 128, and disabled Gemma uses budget zero. Third-generation Pro and Flash keep
their level policies. Browser raw/simple API-key calls use the same cloud projection;
a keyless call with supplied scope reports `An API Key must be set when running in a browser`.

## Native credentials

`ApiClient::new(base_url, api_version, api_key, headers)` selects the authentication once:
`Some(key)`, including an empty key, uses that key and never starts application-default
credentials; `None` selects them. The constructor reads nothing and starts no work.
`ApiClient::prepare_request` is asynchronous: it validates the URL and authored headers first,
then acquires credentials, then inserts the generated authorization (or quota) header only when
the same header name, in any case, was not authored. Authentication still runs when both are
authored.

Native credentials resolve lazily, on the first preparation, and the successful owner is
retained for later requests; a failed resolution is not cached, except that the metadata
probe result, a negative one included, is kept once per process. Policy and residency are
checked before using that result. Discovery order:

1. `GOOGLE_APPLICATION_CREDENTIALS`, then `google_application_credentials`, when nonempty;
2. the platform's well-known application-default file (`HOME/.config/gcloud` or `APPDATA\gcloud`);
3. the metadata server, when `METADATA_SERVER_DETECTION` admits it.

A selected file is canonicalized, must be a regular file and is parsed once; a failure there
never falls through to a lower source. Supported files are authorized user, service account,
impersonated service account, external account (file, URL, AWS and executable sources;
certificate sources) and external-account authorized user. A nonempty
`GOOGLE_CLOUD_QUOTA_PROJECT` replaces the file's quota project on the outer credential only.
An impersonated file keeps its source credentials as a typed handle and takes its target from
the last segment of `service_account_impersonation_url`.

Maestro owns token acquisition for three sources: external-account authorized user files,
certificate external accounts and the metadata server. Their tokens refresh on use, five
minutes before their known expiry, and a token whose expiry is already reached is rejected
rather than sent. Concurrent callers share one in-flight acquisition. Workforce refresh tokens
rotate and the rotated value is kept even when the access token it came with is rejected.
The credential library refreshes the other supported files (authorized user, service account,
impersonated service account and the remaining external-account sources) under its own
caching rules.

Certificate sources read their configuration from `certificate_config_location`, then
`GOOGLE_API_CERTIFICATE_CONFIG`, then `CLOUDSDK_CONFIG` or the platform gcloud directory. The
leaf is prepended to the trust chain when absent, a late leaf is rejected, and the token
exchange runs over HTTPS with the client certificate and no environment proxy.

Metadata detection honors `none` (always disabled) before residency. Otherwise, Google hosts
are admitted without probing, even with an unknown detection value. Off Google hosts it honors
`assume-present`, `bios-only` and `ping-only`, and rejects any other value. `GCE_METADATA_IP`
takes precedence over `GCE_METADATA_HOST`, and a scheme in either is kept. Only `ping-only`
(the default) probes, and only off Google hosts; `DETECT_GCP_RETRIES` sets the probe's
no-response retries. Off Google hosts each metadata request, the probe and the token exchange
alike, waits at most 3000 ms per attempt; on Google hosts token exchanges have no deadline.
Residency comes from serverless variables, Linux DMI data and `/sys/class/net/*/address`; other
platforms' network data is tracked in
[#1188](https://github.com/Orchestration-Maestro/maestro/issues/1188).

Failures expose only a fixed `Authentication: ...` message (acquisition, certificate or
configuration), whether the failure is transient, and the HTTP status when one exists;
tokens, keys, response bodies and credential URLs never appear in any rendering.

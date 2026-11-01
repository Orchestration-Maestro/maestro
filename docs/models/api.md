# Model invocation library

`maestro-models` invokes model protocols through replaceable callbacks. It owns
request preparation, streaming and account primitives, not an agent loop, tool
executor or credential store. Examples below are offline Rust tests using the
same public operations as applications. For topic navigation and current inputs,
see [Current model behavior](../release_behavior/changelog.md).

## Installation and offline quick start

Use a workspace checkout, not a presumed published release:

```toml
[dependencies]
maestro-models = { path = "../maestro/crates/maestro-models" }
```

Adjust the path to your checkout. Default native bundled adapters require a Tokio runtime
with time and I/O drivers enabled, for example a current-thread runtime built
with `enable_all()`. A pure custom callback does not establish that native
adapters work without this runtime.

Read the complete [quick-start test](../../crates/maestro-models/tests/model_guide_examples/streams.rs),
`maestro_guide_quick_start_completes_without_network`. It selects
`openai/gpt-4o-mini`, uses that descriptor's actual API, injects a finite response,
and checks text, stop reason and usage through all four entry points. The shared
[example data](../../crates/maestro-models/tests/model_guide_examples/mod.rs)
and [controlled transport](../../crates/maestro-models/tests/support/bundled.rs)
replace HTTP effects, not protocol parsing. No live provider or secret is needed.

Run all eleven examples from the checkout:

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped cargo test --locked -p maestro-models --test integration model_guide_examples::
```

Build the complete public rustdoc with `cargo doc --no-deps -p maestro-models`.
The [crate-root record guide](../records.md) links the focused contracts; the
source links below lead to the same documented record declarations.

## Models, APIs and providers

These are different concepts:

- A **model descriptor** supplies ID, API, provider, endpoint, capabilities,
  rates and compatibility. It can be authored by a caller.
- A **catalog** supplies owned descriptors through `get_model`, `get_models`
  and `get_providers`. Exact unknown model lookup returns `None`; an unknown
  provider has no models. Descriptor identity comparison uses provider and ID.
- An **API registration** pairs an API identifier with raw/simple callbacks.
  Invocation resolves the descriptor's API, not catalog membership.

The [custom descriptor example](../../crates/maestro-models/tests/model_guide_examples/providers.rs),
`maestro_guide_invokes_custom_descriptors`, verifies lookup and sends an authored
model through an existing protocol to a controlled endpoint. It inspects URL,
headers, system-role selection, token-limit field and omitted compatibility
fields. Copy the descriptor's real protocol; an arbitrary endpoint does not
change its wire format. See [catalog lookup and thinking](catalog.md),
[compatible chat endpoints](chat-completions.md) and
[model generation](generation.md). Ordinary Cargo builds remain offline;
explicit generation commands acquire feeds.

### Bundled protocols and direct operations

The registry initially contains nine protocols in the following order. Catalog
membership is separate: a catalog can contain descriptors whose API is not
registered. The scripted API is opt-in.

All module paths below are under `maestro_models::providers`. Each module's
rustdoc documents its typed options and direct raw/simple operations. Root
forwarding is not interchangeable with the provider-module simple functions:
see [linkage and setup-error boundaries](module-linkage.md).

| API | Owning module / options | Focused contract / runtime witness |
|---|---|---|
| `anthropic-messages` | `messages::anthropic` / `AnthropicOptions` | [Messages](messages.md); `bundled_dispatch::maestro_public_routes_reach_real_adapters` |
| `openai-completions` | `chat::openai_completions` / `OpenAICompletionsOptions` | [Chat completions](chat-completions.md); same dispatch witness |
| `mistral-conversations` | `reasoning::mistral` / `MistralOptions` | [Reasoning conversations](reasoning-conversations.md); same dispatch witness |
| `openai-responses` | `responses::openai_responses` / `OpenAIResponsesOptions` | [Responses](responses.md); same dispatch witness |
| `azure-openai-responses` | `responses::azure_openai_responses` / `AzureOpenAIResponsesOptions` | [Responses](responses.md); `bundled_options::maestro_azure_extras_select_target` |
| `openai-codex-responses` | `response_sessions::openai_codex_responses` / `OpenAICodexResponsesOptions` | [Response sessions](response-sessions.md); `bundled_dispatch::maestro_response_session_cache_affinity_completes` |
| `google-generative-ai` | `content::google` / `GoogleOptions` | [Content generation](content-generation.md); `content_generation_invocation::raw_request_projects_the_selected_payload` |
| `google-vertex` | `content::google_vertex` / `GoogleVertexOptions` | [Cloud content generation](content-generation.md#cloud-invocation); `cloud_content_invocation::cloud_root_and_registry_entries_dispatch` |
| `bedrock-converse-stream` | `cloud::amazon_bedrock` / `BedrockOptions` | [Cloud conversations](cloud-conversations.md); `cloud_conversation_streams::text_events_keep_shared_result_and_completion` |

Each module exposes `stream_*` and `stream_simple_*` under its provider name;
the cloud-conversation functions use `stream_bedrock` and `stream_simple_bedrock`
in their dedicated module. They are not added as root convenience operations.
Runtime witnesses named here are in the existing
[integration target](../../crates/maestro-models/tests/integration.rs).
Qualification checks controlled wire conversion, not live authentication,
reasoning quality, cache-hit rates or billing.

### Not available yet

- The standalone model-account executable and its terminal console: [tracked work](https://github.com/Orchestration-Maestro/maestro/issues/249). The account command library itself is delivered; see [standalone account commands](../model_login.md).

Cloud content generation and cloud conversations in the table are distinct protocols.

## Conversation and JSON records

Use existing typed records rather than a second application schema. Read the
full documented declarations for
[messages and content](../../crates/maestro-request/src/types/messages.rs),
[models and compatibility](../../crates/maestro-request/src/types/models.rs),
[usage](../../crates/maestro-request/src/types/messages.rs),
[events](../../crates/maestro-request/src/types/events.rs) and
[invocation options](../../crates/maestro-models/src/records/types/options.rs).
They are re-exported through `maestro_models` for callers.

`Context` carries an optional system prompt, ordered messages and optional tool
declarations. `Message` is `User`, `Assistant` or `ToolResult`. A user's
`UserContent` is text or blocks; `UserBlock` is text or image. Images carry base64
data and MIME type. `AssistantContent` is text, thinking or tool call, with the
record's optional signatures/redaction. Assistant records also carry protocol,
provider/model identity, usage, stop reason, timestamp, optional response identity,
error and diagnostics. Tool results carry call ID, tool name, text/image content,
optional generic details and the error flag. A `Tool<TParameters>` is a named
description and parameter schema, not an executable callback.

The [history example](../../crates/maestro-models/tests/model_guide_examples/providers.rs),
`maestro_guide_projects_and_serializes_history`, decodes and serializes a complete
context including image data, signed text/thinking and explicit null tool-result
details. It also checks actual continuation wire output. Preserve these records
for persistence rather than reconstructing them from displayed text.

Absent optional fields, explicit null where a selected record admits it, and
explicit false/zero/empty values are different inputs. Do not infer omission
rules from another record. See [record serialization](../records.md),
[raw extra-field presence](module-linkage.md#raw-extras) and
[JSON boundaries](../json.md). `JsonObject` is used for genuinely open JSON data;
retained hooks and typed clients live outside JSON in `ProviderObjects`.

## Streaming and completion

| Generic entry point | Input options | Observation |
|---|---|---|
| `stream` | `Option<ProviderStreamOptions>` | `Result<AssistantMessageEventStream, DiagnosticErrorInfo>` |
| `stream_simple` | `Option<SimpleStreamOptions>` | Same stream/result boundary |
| `complete` | Raw options | Future of setup result and shared final assistant |
| `complete_simple` | Simple options | Same completion boundary |

All four start invocation immediately. Completion places setup failure inside
its returned future and observes the result without consuming event iteration.
A stream's `next()` consumes one queued event; multiple readers compete for
those events. Its `result()` independently observes the shared terminal assistant.
Dropping an observer does not cancel request work. `end(None)` closes iteration
but does not invent a result; producers need an actual result to settle it.

### All twelve event variants

Rust fields use snake case; event JSON uses `contentIndex` and `toolCall`.
`partial`, `message` and `error` are live shared assistant handles, not frozen
snapshots. Read or clone their current data without keeping a lock across await.

| Rust variant | Serialized `type` | Fields |
|---|---|---|
| `Start` | `start` | `partial` |
| `TextStart` | `text_start` | `content_index`, `partial` |
| `TextDelta` | `text_delta` | `content_index`, `delta`, `partial` |
| `TextEnd` | `text_end` | `content_index`, `content`, `partial` |
| `ThinkingStart` | `thinking_start` | `content_index`, `partial` |
| `ThinkingDelta` | `thinking_delta` | `content_index`, `delta`, `partial` |
| `ThinkingEnd` | `thinking_end` | `content_index`, `content`, `partial` |
| `ToolcallStart` | `toolcall_start` | `content_index`, `partial` |
| `ToolcallDelta` | `toolcall_delta` | `content_index`, `delta`, `partial` |
| `ToolcallEnd` | `toolcall_end` | `content_index`, `tool_call`, `partial` |
| `Done` | `done` | `reason`, `message` |
| `Error` | `error` | `reason`, `error` |

Use `content_index`, not an assumption that blocks are contiguous. The
[interleaved consumer](../../crates/maestro-models/tests/model_guide_examples/streams.rs),
`maestro_guide_consumes_interleaved_stream`, checks event order, independent
thinking/text/tool indices and the same live final handle. The failure example
covers the remaining `error` event. A `ToolcallEnd` contains parsed/repaired
arguments, **not execution-validated arguments**.

## Schemas, tool progress and execution

Three complete [tool examples](../../crates/maestro-models/tests/model_guide_examples/tools.rs)
show declaration, progress and the caller's execution loop:

1. `maestro_guide_builds_portable_tool_schemas`: JSON Schema plus `string_enum`,
   with absent, nonempty and empty description/default options. The helper does
   not produce an `anyOf` wrapper.
2. `maestro_guide_reads_partial_arguments_without_execution`: read empty,
   incomplete and completed JSON arguments at gated deltas. Progress is for
   display, never a reason to execute. Not every adapter streams fields at the
   same granularity.
3. `maestro_guide_validates_before_tool_execution`: obtain a completed invalid
   `kelvin` enum from a real controlled adapter, reject it before any tool effect,
   and return caller-authored error feedback in a tool result before continuing.

Call `validate_tool_call` to find the declaration by exact name and validate, or
`validate_tool_arguments` when you already selected the declaration. Execute
only the returned checked/coerced object. A schema default is not automatically
inserted. Validation failure belongs to the caller's tool loop; it does not
turn the provider stream into an error. Successful execution similarly produces
a caller-authored `ToolResult` associated with the returned call ID. See the
[argument checker](arguments.md) for supported schemas, coercion, formats and
exact path/received-argument feedback. An application still owns authorization
for any tool effect.

## Images and cross-provider histories

Projection is model-sensitive: replay compatibility checks provider, API **and
model ID**. Compatible thinking can retain signatures; foreign ordinary thinking
becomes unsigned plain text, not guaranteed tagged text. Blank foreign thinking
is omitted under the authored-whitespace rule: U+FEFF is blank, U+0085 is not.
For nonvision models, user images can become
`(image omitted: model does not support images)`; tool images can become
`(tool image omitted: model does not support images)`.

`transform_messages` borrows stored messages and returns an owned request view.
Adapter wire conversion can further regroup content and normalize tool IDs.
Do not promise that same-provider histories, user messages or tool results pass
through unchanged. Failed assistant attempts are omitted from replay. Missing
results and normalized-ID collisions are handled by original call occurrence,
not by treating normalized IDs as unique source identities.

The history example exercises eight destination projections and a persisted
context, then sends an actual returned assistant plus its tool feedback to a
different provider identity and checks the serialized request. Full association
and image policy remain in [conversation projection](../conversation-projection.md),
including the existing `maestro_projection_preserves_colliding_call_associations`
and `maestro_projection_downgrades_unsupported_images_once` regressions in
[history tests](../../crates/maestro-models/tests/model_history_projection.rs).

## Thinking, usage and provider-specific options

`SimpleStreamOptions` carries common options, requested reasoning, optional
per-level `ThinkingBudgets` and tool choice. Raw options carry common settings,
open adapter fields and typed retained objects. The
[thinking/options example](../../crates/maestro-models/tests/model_guide_examples/providers.rs),
`maestro_guide_selects_thinking_and_transport_options`, shows a nonreasoning
model selecting off, unsupported low selecting medium, and independently chosen
raw/simple controls reaching real requests. A missing thinking-map entry and
an explicit null entry are not equivalent: see [catalog thinking levels](catalog.md).
Streaming thinking is content with its own indices; it is not final answer text.

`StreamOptions::default()` does not define a universal timeout, retry or cache
policy. Empty options leave choices to the selected adapter. Consult each
focused guide for defaults, header precedence, token limits, cache handling and
which fields are unused. Explicit zero/false/empty values are not automatically
interchangeable with absence. Provider-specific extras and nullable fields are
listed in [module linkage](module-linkage.md#raw-extras).

Reported `Usage` keeps input, output, cache-read, cache-write and total tokens;
`UsageCost` has each category and total. The total keeps the presence of the saved
count: a report that omits it leaves it absent rather than zero, and a supplied
number, zero included, is kept as supplied. `calculate_cost` uses the supplied
model's USD-per-million rates; it estimates, rather than certifies, billing.
Do not double-count cached categories or reinterpret an adapter's reported
total. See [accounting](accounting.md) and its controlled usage/abort tests.

## Errors, cancellation, hooks and diagnostics

Successful stop literals are `stop`, `length` and `toolUse`; failure literals
are `error` and `aborted`. `DoneReason` and `ErrorReason` restrict terminal event
reasons accordingly. Setup errors are distinct from terminal stream failures:
a custom callback can fail immediately; bundled registration settles adapter
setup failures into error streams. Direct module simple functions can return
missing-key setup errors before creating a stream; root simple forwarding settles
them. Mode-aware registered entries (`stream_for`, `stream_simple_for`,
`complete_for`, `complete_simple_for`) require the caller's submission facility and
retain local hooks in the selected producer. The default three-argument entries
keep their threaded options and provider-owned default admission. See [linkage](module-linkage.md).

The [failure and cancellation example](../../crates/maestro-models/tests/model_guide_examples/streams.rs),
`maestro_guide_observes_failure_and_cancellation`, cancels only after visible text
and a pending-body-read handshake. It checks the terminal reason, retained text
and reported usage, then continues with the actual returned partial assistant.
The request omits the aborted attempt; stored history can still retain it.
Cancellation stops local work, not remote effects. For message-protocol injected
clients, reads end on their own and cancellation is checked before each read;
HTTP transport reads race cancellation. Do not assume identical body ownership.

Payload hooks receive and return the selected `PayloadHookInput`. Its JSON can
be kept, edited or replaced; both public and cloud content generation consume the edited
signal. Content generation does not use the response hook; cloud conversations
invoke it after the first known service event or successful end of stream,
including empty or unknown-only streams. A failure before either prevents the
hook from running. Other policies remain adapter owned. Read [hook contracts](../records.md), the [content-generation exceptions](content-generation.md),
[cloud SDK configuration](cloud-conversations.md) and
[response-session callback/retry policy](response-sessions.md), rather than
assuming a universal callback boundary.

`create_assistant_message_diagnostic` constructs a diagnostic with a current
timestamp; `append_assistant_message_diagnostic` appends supplied details without
replacing existing diagnostics. Read [diagnostic declarations](../../crates/maestro-request/src/diagnostics.rs)
and the `maestro_diagnostics_append_with_current_timestamp` witness in
[record tests](../../crates/maestro-models/tests/model_records.rs).

## Native and browser authentication

Explicit request keys, environment discovery and account lifecycle are separate.
`find_env_keys` reports configured environment sources; `get_env_api_key` resolves
one under provider-specific precedence. An ambient-configuration marker is not a
usable secret, login or proof of an invocable protocol. Empty and whitespace keys
have provider-specific handling; see [request authentication](../request-authentication.md)
for actual variables, explicit-key precedence and endpoint environment policy.

Browser targets have no process environment discovery. Supply an explicit key
or an application-controlled proxy/injected client, considering browser origin
and credential exposure policy. The extended
[browser import example](../../crates/maestro-models/examples/browser_import_check.rs)
compiles typed model, context, tool, options and a retained local client with an
explicit key. It does not start a request, native callback listener, cloud
credential flow or native socket. `maestro_browser_environment_has_no_key`
builds this actual wasm artifact: this is compile evidence, not browser execution
or live authorization qualification.

## Account flows and registration

The OAuth registry starts with `anthropic`, `github-copilot` and `openai-codex`.
Use the existing [subscription](oauth.md#subscription-accounts),
[device](oauth.md#device-accounts) and [response-account](oauth.md#response-accounts)
login/refresh functions and `OAuthLoginCallbacks` interaction contract. Library
login does not open a browser or persist credentials for the caller.

The [supplied-account example](../../crates/maestro-models/tests/model_guide_examples/accounts.rs),
`maestro_guide_refreshes_supplied_account_credentials`, runs in a fresh process
with owned fake state. It exercises authorization/prompt callbacks, nonexpired
extraction without refresh, expired extraction with one refresh, missing supplied
credentials and returned updated credentials. `get_oauth_api_key` returns an
`OAuthApiKey`; `refresh_oauth_token` delegates refresh directly without an expiry
check. Callers store the result. The OAuth guide documents failure boundaries
and the existing controlled native-flow witnesses.

`maestro_guide_registration_keeps_existing_handles` distinguishes API callbacks
from account providers. Replacement preserves a slot and retained old handles
continue working after registry changes. Source-based API removal only removes
entries still owned by that source. API clear stays empty until explicit
registration/reset; reset restores the nine bundled entries. OAuth unregister
restores a built-in implementation or removes a custom entry; OAuth reset restores
the original three-provider membership. No parent environment is changed by these
fresh-child examples.

## Cache, sessions, cleanup and scripted models

Cache retention literals are `none`, `short` and `long`. Transport literals are
`sse`, `websocket`, **`websocket-cached`** and `auto`; enum selection alone does
not prove an adapter supports a transport or reuses a connection.
[Response sessions](response-sessions.md) documents missing-transport `auto`,
pre-start fallback, session affinity, cached-context reuse, idle cleanup,
explicit close and debug counters. It names the native socket qualifications;
`maestro_socket_completions_continue_with_actual_assistants` additionally checks
continuation in [streaming socket tests](../../crates/maestro-models/tests/model_streaming/socket.rs).
Do not copy these session policies to other adapters.

`register_session_resource_cleanup` returns an explicit removal handle; dropping
that handle does not remove the registration. `cleanup_session_resources`
invokes the live registered callbacks and collects failures in order. Call it
for the owned session or all sessions as appropriate. See
[cleanup declarations](../../crates/maestro-models/src/records/session_resources.rs),
`maestro_cleanup_collects_failures_in_order` in
[invocation tests](../../crates/maestro-models/tests/model_invocation.rs) and
`maestro_cleanup_observes_live_changes` in
[lifetime tests](../../crates/maestro-models/tests/model_stream_lifetimes.rs).

The [scripted model guide](faux.md) documents opt-in `register_faux_provider`,
message/content builders, queued response replacement/appending, pending/call
counts, pacing, factories, session-local cache and explicit unregister. Reuse the
executing two-turn `maestro_documented_flow_runs_offline` in
[scripted invocation tests](../../crates/maestro-models/tests/scripted_model_invocation.rs),
not a second simulator or tutorial executor. A pre-aborted scripted call still
consumes its queued response; dropping a result observer is not cancellation.

## Contributing and license

Follow the existing [provider contribution checklist](../../AGENTS.md#provider-contribution-checklist),
[development checks](../development.md) and [catalog generation](generation.md).
Use controlled protocol fixtures and existing owned qualification tests, not
nonexistent test-file recommendations or live paid services. Ordinary builds do
not acquire model feeds. Public rustdoc and focused feature pages accompany
changes; release automation owns history, so pull requests do not edit it.

Maestro is distributed under the [MIT license](../../LICENSE), including the
attribution retained there.

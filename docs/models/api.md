# Model invocation library

`maestro-models` invokes model protocols through replaceable callbacks. It
prepares requests, streams responses and provides account primitives. It has no
agent loop, tool executor or credential store. Every example below is an offline
Rust test that calls the same public operations an application calls. For topic
navigation and current inputs, see
[Current model behavior](../release_behavior/changelog.md).

## Installation and offline quick start

Depend on a workspace checkout; no published release is assumed:

```toml
[dependencies]
maestro-models = { path = "../maestro/crates/maestro-models" }
```

Change the path to match your checkout. The default native bundled adapters need
a Tokio runtime with the time and I/O drivers enabled, for example a
current-thread runtime built with `enable_all()`. A pure custom callback working
without a runtime says nothing about the native adapters.

The [quick-start test](../../crates/maestro-models/tests/model_guide_examples/streams.rs),
`maestro_guide_quick_start_completes_without_network`, is the complete example.
It selects `openai/gpt-4o-mini`, uses that descriptor's own API, injects a finite
response and checks text, stop reason and usage through all four entry points.
The shared [example data](../../crates/maestro-models/tests/model_guide_examples/mod.rs)
and [controlled transport](../../crates/maestro-models/tests/support/bundled.rs)
stand in for HTTP; protocol parsing still runs. It needs no live provider and no
secret.

Run all eleven examples from the checkout:

```sh
cargo test --locked -p maestro-models --test integration model_guide_examples::
```

Build the public rustdoc with `cargo doc --no-deps -p maestro-models`. The
[crate-root record guide](../records.md) links the focused contracts, and the
source links below lead to the same documented record declarations.

## Models, APIs and providers

Three separate concepts:

- A **model descriptor** holds the ID, API, provider, endpoint, capabilities,
  rates and compatibility. A caller can author one.
- A **catalog** owns descriptors and serves them through `get_model`,
  `get_models` and `get_providers`. Looking up an unknown model returns `None`,
  and an unknown provider has no models. Two descriptors are the same model when
  provider and ID match.
- An **API registration** pairs an API identifier with raw and simple callbacks.
  Invocation resolves the descriptor's API, never its catalog membership.

The [custom descriptor example](../../crates/maestro-models/tests/model_guide_examples/providers.rs),
`maestro_guide_invokes_custom_descriptors`, checks lookup and sends an authored
model through an existing protocol to a controlled endpoint. It inspects the URL,
headers, system-role selection, token-limit field and omitted compatibility
fields. Copy the descriptor of the protocol you target: a different endpoint does
not change the wire format. See [catalog lookup and thinking](catalog.md),
[compatible chat endpoints](chat-completions.md) and
[model generation](generation.md). Ordinary Cargo builds stay offline; only the
explicit generation commands acquire feeds.

### Bundled protocols and direct operations

The registry starts with the nine protocols below, in this order. Catalog
membership is separate: a catalog can hold descriptors whose API is not
registered. The scripted API is opt-in.

Every module path below sits under `maestro_models::providers`. Each module's
rustdoc documents its typed options and its direct raw and simple operations. The
root forwarding functions and the provider-module simple functions differ; see
[linkage and setup-error boundaries](module-linkage.md).

| API | Owning module / options | Focused contract / runtime witness |
|---|---|---|
| `anthropic-messages` | `messages::endpoint` / `AnthropicOptions` | [Messages](messages.md); `bundled_dispatch::maestro_public_routes_reach_real_adapters` |
| `openai-completions` | `chat::endpoint` / `OpenAICompletionsOptions` | [Chat completions](chat-completions.md); same dispatch witness |
| `mistral-conversations` | `reasoning::endpoint` / `MistralOptions` | [Reasoning conversations](reasoning-conversations.md); same dispatch witness |
| `openai-responses` | `responses_api::openai` / `OpenAIResponsesOptions` | [Responses](responses.md); same dispatch witness |
| `azure-openai-responses` | `responses_api::azure_transport` / `AzureOpenAIResponsesOptions` | [Responses](responses.md); `bundled_options::maestro_azure_extras_select_target` |
| `openai-codex-responses` | `response_sessions::codex` / `OpenAICodexResponsesOptions` | [Response sessions](response-sessions.md); `bundled_dispatch::maestro_response_session_cache_affinity_completes` |
| `google-generative-ai` | `content::generative` / `GoogleOptions` | [Content generation](content-generation.md); `content_generation_invocation::raw_request_projects_the_selected_payload` |
| `google-vertex` | `content::managed` / `GoogleVertexOptions` | [Cloud content generation](content-generation.md#cloud-invocation); `cloud_content_invocation::cloud_root_and_registry_entries_dispatch` |
| `bedrock-converse-stream` | `cloud::amazon_bedrock` / `BedrockOptions` | [Cloud conversations](cloud-conversations.md); `cloud_conversation_streams::text_events_keep_shared_result_and_completion` |

Each module exposes `stream_*` and `stream_simple_*` under its provider name.
The cloud-conversation functions are `stream_bedrock` and
`stream_simple_bedrock`, in their own module. None is a root convenience
operation. The runtime witnesses named here live in the
[integration target](../../crates/maestro-models/tests/integration.rs).
Qualification checks controlled wire conversion. It does not check live
authentication, reasoning quality, cache-hit rates or billing.

### Not available yet

- The standalone model-account executable and its terminal console: [tracked work](https://github.com/Orchestration-Maestro/maestro/issues/249). The account command library is delivered; see [standalone account commands](../model_login.md).

In the table, cloud content generation and cloud conversations are distinct
protocols.

## Conversation and JSON records

Use the existing typed records; do not define a second application schema. The
documented declarations are in
[messages and content](../../crates/maestro-request/src/types/messages.rs),
[models and compatibility](../../crates/maestro-request/src/types/models.rs),
[usage](../../crates/maestro-request/src/types/messages.rs),
[events](../../crates/maestro-request/src/types/events.rs) and
[invocation options](../../crates/maestro-models/src/records/types/options.rs).
`maestro_models` re-exports them.

`Context` holds an optional system prompt, ordered messages and optional tool
declarations. `Message` is `User`, `Assistant` or `ToolResult`.

- A user's `UserContent` is text or blocks. A `UserBlock` is text or an image; an
  image carries base64 data and a MIME type.
- `AssistantContent` is text, thinking or a tool call, each with the record's
  optional signatures and redaction. An assistant record also carries the
  protocol, the provider and model identity, usage, stop reason, timestamp, an
  optional response identity, the error and diagnostics.
- A tool result carries the call ID, tool name, text or image content, optional
  generic details and the error flag.
- A `Tool<TParameters>` is a named description with a parameter schema. It is not
  an executable callback.

The [history example](../../crates/maestro-models/tests/model_guide_examples/providers.rs),
`maestro_guide_projects_and_serializes_history`, decodes and serializes a full
context: image data, signed text and thinking, and explicit null tool-result
details. It also checks the wire output of a continuation. Persist these records
as they are; do not rebuild them from displayed text.

An absent optional field, an explicit null (where the record admits one) and an
explicit false, zero or empty value are three different inputs. Each record has
its own omission rules, so do not carry them over from another record. See
[record serialization](../records.md),
[raw extra-field presence](module-linkage.md#raw-extras) and
[JSON boundaries](../json.md). `JsonObject` holds open-ended JSON data.
Retained hooks and typed clients live outside JSON, in `ProviderObjects`.

## Streaming and completion

| Generic entry point | Input options | Observation |
|---|---|---|
| `stream` | `Option<ProviderStreamOptions>` | `Result<AssistantMessageEventStream, DiagnosticErrorInfo>` |
| `stream_simple` | `Option<SimpleStreamOptions>` | Same stream/result boundary |
| `complete` | Raw options | Future of setup result and shared final assistant |
| `complete_simple` | Simple options | Same completion boundary |

All four start the invocation at once.

- `complete` and `complete_simple` return setup failures inside the returned
  future. They read the result without consuming events from the stream.
- `next()` takes one queued event from the stream; several readers compete for
  those events.
- `result()` reads the shared terminal assistant independently of `next()`.
- Dropping an observer does not cancel the request work.
- `end(None)` closes iteration but supplies no result. A producer must supply a
  result to settle the stream.

### All twelve event variants

Rust fields use snake case; event JSON uses `contentIndex` and `toolCall`.
`partial`, `message` and `error` are live shared assistant handles, not
snapshots. Read or clone their current data, and release the lock before any
`await`.

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

Address blocks by `content_index`; do not assume they are contiguous. The
[interleaved consumer](../../crates/maestro-models/tests/model_guide_examples/streams.rs),
`maestro_guide_consumes_interleaved_stream`, checks event order, independent
thinking, text and tool indices, and the same live final handle. The failure
example covers the `error` event. A `ToolcallEnd` carries parsed and repaired
arguments, **not execution-validated arguments**.

## Schemas, tool progress and execution

Three [tool examples](../../crates/maestro-models/tests/model_guide_examples/tools.rs)
cover declaration, progress and the caller's execution loop:

1. `maestro_guide_builds_portable_tool_schemas` builds JSON Schema with
   `string_enum`, for absent, nonempty and empty description and default options.
   The helper adds no `anyOf` wrapper.
2. `maestro_guide_reads_partial_arguments_without_execution` reads empty,
   incomplete and completed JSON arguments at gated deltas. Show progress to the
   user; never execute on it. Adapters differ in how finely they stream fields.
3. `maestro_guide_validates_before_tool_execution` takes a completed invalid
   `kelvin` enum from a real controlled adapter, rejects it before any tool
   effect, and returns caller-authored error feedback in a tool result before the
   run continues.

Call `validate_tool_call` to find the declaration by exact name and validate in
one step. Call `validate_tool_arguments` when you have already selected the
declaration. Execute only the checked and coerced object that comes back. A schema
default is not inserted automatically. A validation failure belongs to the
caller's tool loop and does not turn the provider stream into an error. A
successful execution likewise yields a caller-authored `ToolResult` tied to the
returned call ID. The [argument checker](arguments.md) lists the supported
schemas, coercion, formats and the exact path and received-argument feedback. The
application still owns authorization for every tool effect.

## Images and cross-provider histories

Projection depends on the target model. Replay compatibility checks the provider,
the API **and the model ID**.

- Compatible thinking can keep its signatures.
- Foreign ordinary thinking becomes unsigned plain text. It is not guaranteed to
  carry tags.
- Blank foreign thinking is omitted under the authored-whitespace rule: U+FEFF is
  blank, U+0085 is not.
- For a model without vision, a user image can become
  `(image omitted: model does not support images)` and a tool image can become
  `(tool image omitted: model does not support images)`.

`transform_messages` borrows the stored messages and returns an owned request
view. Adapter wire conversion can regroup content further and normalize tool IDs,
so a same-provider history, user message or tool result may not pass through
unchanged. Failed assistant attempts are omitted from replay. Missing results and
normalized-ID collisions resolve by the original call occurrence; a normalized ID
is not a unique identity for its source.

The history example runs eight destination projections and a persisted context.
It then sends a returned assistant message and its tool feedback to a different
provider identity and checks the serialized request. The full association and
image policy is in [conversation projection](../conversation-projection.md),
with the regressions `maestro_projection_preserves_colliding_call_associations`
and `maestro_projection_downgrades_unsupported_images_once` in the
[history tests](../../crates/maestro-models/tests/model_history_projection.rs).

## Thinking, usage and provider-specific options

`SimpleStreamOptions` holds the common options, the requested reasoning, optional
per-level `ThinkingBudgets` and the tool choice. Raw options hold the common
settings, open adapter fields and typed retained objects. The
[thinking and options example](../../crates/maestro-models/tests/model_guide_examples/providers.rs),
`maestro_guide_selects_thinking_and_transport_options`, shows a model without
reasoning selecting off, an unsupported low level selecting medium, and
independently chosen raw and simple controls reaching real requests. A missing
thinking-map entry and an explicit null entry are different; see
[catalog thinking levels](catalog.md). Streamed thinking is content with its own
indices, not part of the final answer text.

`StreamOptions::default()` sets no universal timeout, retry or cache policy.
Empty options leave each choice to the selected adapter. The focused guides give
the defaults, header precedence, token limits, cache handling and unused fields
for each adapter. Do not assume an explicit zero, false or empty value is equivalent
to an absent one. Provider-specific extras and nullable fields are listed in
[module linkage](module-linkage.md#raw-extras).

`Usage` reports input, output, cache-read, cache-write and total tokens, and
`UsageCost` reports each category and the total. The total keeps whether a count
was saved: a report that omits it leaves it absent, not zero, and a supplied
number, zero included, is kept as supplied. `calculate_cost` applies the supplied
model's USD-per-million rates and estimates billing; it does not certify it.
Do not count cached categories twice, and do not reinterpret the total an adapter
reports. See [accounting](accounting.md) and its controlled usage and abort tests.

## Errors, cancellation, hooks and diagnostics

The success stop literals are `stop`, `length` and `toolUse`; the failure
literals are `error` and `aborted`. `DoneReason` and `ErrorReason` limit the
reasons a terminal event can carry to those sets.

A setup error differs from a terminal stream failure:

- A custom callback can fail immediately.
- Bundled registration settles adapter setup failures into error streams.
- A direct module simple function can return a missing-key setup error before it
  creates a stream. Root simple forwarding settles the same error into a stream.

The mode-aware registered entries (`stream_for`, `stream_simple_for`,
`stream_simple_via_for`, `complete_for`, `complete_simple_for`) need the caller's
submission facility and keep local hooks in the selected producer. The default
three-argument entries keep their threaded options and the provider's own default
admission. See [linkage](module-linkage.md).

The [failure and cancellation example](../../crates/maestro-models/tests/model_guide_examples/streams.rs),
`maestro_guide_observes_failure_and_cancellation`, cancels only after visible text
and a pending-body-read handshake. It checks the terminal reason, the retained
text and the reported usage, then continues with the partial assistant message it
received. The next request omits the aborted attempt, while stored history can
keep it. Cancellation stops local work; it does not undo remote effects. With an
injected client for the message protocol, reads end on their own and cancellation
is checked before each read. HTTP transport reads race cancellation. Do not assume
the two own the body the same way.

Payload hooks receive and return the selected `PayloadHookInput`. The hook can
keep, edit or replace its JSON, and both public and cloud content generation use
the edited signal. Content generation does not call the response hook. Cloud
conversations call it after the first known service event or a successful end of
stream, including an empty stream or one with only unknown events; a failure
before either point keeps the hook from running. Each adapter owns its other
policies. No callback boundary is universal: read the
[hook contracts](../records.md), the
[content-generation exceptions](content-generation.md),
[cloud SDK configuration](cloud-conversations.md) and the
[response-session callback and retry policy](response-sessions.md).

`create_assistant_message_diagnostic` builds a diagnostic with the current
timestamp. `append_assistant_message_diagnostic` adds the supplied details and
keeps the existing diagnostics. The declarations are in
[diagnostics](../../crates/maestro-request/src/diagnostics.rs), and the
`maestro_diagnostics_append_with_current_timestamp` witness is in the
[record tests](../../crates/maestro-models/tests/model_records.rs).

## Native and browser authentication

Explicit request keys, environment discovery and the account lifecycle are three
separate things. `find_env_keys` reports the configured environment sources, and
`get_env_api_key` resolves one under the provider's precedence. An
ambient-configuration marker is not a usable secret, a login or proof that a
protocol can be invoked. Providers treat empty and whitespace keys differently;
[request authentication](../request-authentication.md) lists the variables, the
explicit-key precedence and the endpoint environment policy.

Browser targets cannot discover the process environment. Supply an explicit key,
or an application-controlled proxy or injected client, and weigh the browser
origin and credential exposure. The
[browser import witness](../../crates/maestro-models/tests/support/browser_import_check.rs)
compiles a typed model, context, tool, options and a retained local client with
an explicit key. It starts no request, native callback listener, cloud
credential flow or native socket. `maestro_browser_environment_has_no_key` builds
that wasm artifact. This is compile evidence only; it is not a browser run or a
live authorization check.

## Account flows and registration

The OAuth registry starts with `anthropic`, `github-copilot` and `openai-codex`.
Use the [subscription](oauth.md#subscription-accounts),
[device](oauth.md#device-accounts) and [response-account](oauth.md#response-accounts)
login and refresh functions, with the `OAuthLoginCallbacks` interaction contract.
Library login neither opens a browser nor stores credentials for the caller.

The [supplied-account example](../../crates/maestro-models/tests/model_guide_examples/accounts.rs),
`maestro_guide_refreshes_supplied_account_credentials`, runs in a fresh process
with fake state it owns. It covers the authorization and prompt callbacks, key
extraction from unexpired credentials without a refresh, extraction from expired
credentials with one refresh, missing supplied credentials and the updated
credentials that come back. `get_oauth_api_key` returns an `OAuthApiKey`.
`refresh_oauth_token` refreshes directly, without an expiry check. The caller
stores the result. The OAuth guide documents the failure boundaries and the
controlled native-flow witnesses.

`maestro_guide_registration_keeps_existing_handles` separates API callbacks from
account providers:

- Replacing a registration keeps its slot, and old handles you retained keep
  working after registry changes.
- Source-based API removal removes only the entries that source still owns.
- Clearing the API registry leaves it empty until you register or reset
  explicitly. A reset restores the nine bundled entries.
- OAuth unregister restores a built-in implementation or removes a custom entry.
  An OAuth reset restores the original three providers.

These fresh-child examples change nothing in the parent environment.

## Cache, sessions, cleanup and scripted models

Cache retention literals are `none`, `short` and `long`. Transport literals are
`sse`, `websocket`, **`websocket-cached`** and `auto`. Selecting a value does not
prove the adapter supports that transport or reuses a connection.
[Response sessions](response-sessions.md) documents `auto` when a transport is
missing, fallback before the start, session affinity, cached-context reuse, idle
cleanup, explicit close and debug counters, and names the native socket
qualifications. `maestro_socket_completions_continue_with_actual_assistants` also
checks continuation in the
[streaming socket tests](../../crates/maestro-models/tests/model_streaming/socket.rs).
Do not apply these session policies to other adapters.

`register_session_resource_cleanup` returns a removal handle. Dropping the handle
does not remove the registration. `cleanup_session_resources` invokes the live
registered callbacks and collects their failures in order; call it for the one
session you own or for all sessions, as the case requires. See the
[cleanup declarations](../../crates/maestro-models/src/records/sessions.rs),
`maestro_cleanup_collects_failures_in_order` in the
[invocation tests](../../crates/maestro-models/tests/model_invocation.rs) and
`maestro_cleanup_observes_live_changes` in the
[lifetime tests](../../crates/maestro-models/tests/model_stream_lifetimes.rs).

The [scripted model guide](faux.md) covers the opt-in `register_faux_provider`,
the message and content builders, replacing and appending queued responses,
pending and call counts, pacing, factories, the session-local cache and explicit
unregister. Reuse the two-turn `maestro_documented_flow_runs_offline` in the
[scripted invocation tests](../../crates/maestro-models/tests/scripted_model_invocation.rs);
do not write a second simulator or tutorial executor. A scripted call that is
aborted before it starts still consumes its queued response. Dropping a result
observer is not cancellation.

## Contributing and license

Follow the [provider contribution checklist](../../AGENTS.md#provider-contribution-checklist),
the [development checks](../development.md) and [catalog generation](generation.md).
Test with controlled protocol fixtures and the existing qualification tests; use
no live paid services. Ordinary builds do not acquire model feeds. A change ships
with its public rustdoc and focused feature page. Release automation writes the
history, so a pull request does not edit it.

Maestro is distributed under the [MIT license](../../LICENSE), including the
attribution kept there.

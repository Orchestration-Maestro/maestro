# Response endpoints

The standard endpoint is available under
`providers::responses::openai_responses`; `OpenAIResponsesOptions` is also exported
at the package root. Conversion and event reduction remain crate-internal.

## Standard invocation

`stream_openai_responses` reports setup failures through its stream.
`stream_simple_openai_responses` rejects a missing provider key before creating
a stream, then uses the shared simple budget and reasoning selection. Raw calls
also fall back to `OPENAI_API_KEY` after explicit and provider keys.

Cache retention selects an explicit option, otherwise the exact `long` value of
`MAESTRO_CACHE_RETENTION`, otherwise short. Long retention is sent only when the
model compatibility permits it. Disabling `sendSessionIdHeader` leaves generated
`x-client-request-id` enabled for a nonempty cache session. Cache none suppresses
generated affinity and the prompt key, not explicitly supplied headers.

Reasoning options select effort and summary for reasoning models; requested
reasoning includes encrypted content for replay. Summary alone selects literal
medium effort, without consulting the model's effort mapping. Service tier options distinguish
omission, explicit null and named values. Pricing uses the echoed tier unless
it is absent or null, then the requested tier. Flex scales cost by 0.5; priority
uses 2.5 for exactly `gpt-5.5` and 2 for other models.

Payload hooks may replace the request with any non-null JSON value. Replacements
use the shared [compact JSON conversion](arguments.md#streamed-arguments).
A null replacement fails before transport.
Response hooks complete before Start and streamed body processing. If a payload
hook disables streaming, the response body is consumed before the response hook;
a non-event response cannot feed the event reducer.
Transport, metadata and maximum retry-delay preferences are unused here. The
endpoint uses the shared [HTTP transport](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/models/chat-completions.md#transport)
and the conversion and reduction described below.

### Controlled example

Run inside a native Tokio runtime, or a browser executor. This replacement
transport returns a finite response without contacting an external service.

```rust,no_run
use std::sync::Arc;
use maestro_models::{Context, Fetch, HttpResponse, OpenAIResponsesOptions, StreamOptions, get_model};
use maestro_models::providers::responses::openai_responses::stream_openai_responses;

async fn controlled() {
    let fetch: Fetch = Arc::new(|_| {
        let body = b"data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n\n".to_vec();
        Box::pin(std::future::ready(Ok(HttpResponse {
            status: 200, status_text: String::new(), headers: Default::default(),
            body: Box::pin(futures_util::stream::iter([Ok(body)])),
        })))
    });
    let model = get_model("openai", "gpt-5.4").unwrap();
    let context = Context { system_prompt: None, messages: vec![], tools: None };
    let stream = stream_openai_responses(model, context, Some(OpenAIResponsesOptions {
        common: StreamOptions { api_key: Some("controlled-key".into()), fetch: Some(fetch), ..Default::default() },
        ..Default::default()
    }));
    let message = stream.result().await;
    assert!(message.read().is_ok());
}
```

## Cloud invocation

`providers::responses::azure_openai_responses` provides
`stream_azure_openai_responses` and `stream_simple_azure_openai_responses`;
`AzureOpenAIResponsesOptions` is also exported at the package root. Raw setup
failures are stream errors. Simple invocation rejects a missing provider key
before creating a stream and uses the shared simple budgets and reasoning support.

Credentials select a nonempty explicit key, then the provider's environment key.
Raw invocation alone finally tries `AZURE_OPENAI_API_KEY`. Requests authenticate
with `api-key`; model and caller headers override defaults through the existing
case-insensitive layering. Cloud invocation generates no affinity headers.

The base URL selects a trimmed nonblank `azure_base_url`, then
`AZURE_OPENAI_BASE_URL`, then nonempty `azure_resource_name` or
`AZURE_OPENAI_RESOURCE_NAME`, then the descriptor base URL. A selected invalid URL
fails without fallback. Resource shorthand constructs
`https://{resource}.openai.azure.com/openai/v1`. Azure `OpenAI` and Cognitive Services
host roots and `/openai` paths normalize to `/openai/v1` and discard their query.
Other paths retain non-version query pairs, including order, duplicates and
escaped values. `/responses` is appended to the pathname; fragments are removed.
No deployment segment is generated. `azure_api_version`, then
`AZURE_OPENAI_API_VERSION`, then `v1` selects the nonempty version, replacing all
query keys decoding exactly to `api-version`. The inserted value uses RFC3986
encoding, including `%20` for spaces. Resource names and versions are not trimmed.

Deployment selects nonempty `azure_deployment_name`, then the matching nonempty
value of `AZURE_OPENAI_DEPLOYMENT_NAME_MAP`, then the descriptor ID. Map entries
are comma-separated; only the first two equals-separated fields are considered.
Entry trimming precedes field-emptiness checks; field trimming follows them.
Later valid entries replace earlier ones, including a whitespace-only value that
becomes empty. A deployment changes the request body, not the selected model's
identity or rates.

The supplied session ID becomes `prompt_cache_key`, including an empty string,
independently of retention. Cloud bodies omit store, retention, service tier and
metadata. Reasoning uses the shared effort mapping without the standard endpoint's
account-provider exception; summary-only requests select literal `medium`.
Usage uses model rates without service-tier scaling. Hooks, HTTP retries,
timeouts and cancellation use the existing [transport](chat-completions.md#transport),
[JSON conversion](arguments.md#streamed-arguments) and [response reduction](#event-reduction).
Transport and maximum retry-delay preferences are unused here.

### Cloud error messages

Cloud-authored failures use these messages; transport, hook and reduction errors
retain their owning operation's text:

- `Azure OpenAI API key is required. Set AZURE_OPENAI_API_KEY environment variable or pass it as an argument.`
- `No API key for provider: {provider}`
- `Invalid Azure OpenAI base URL: {baseUrl}`
- `Azure OpenAI base URL is required. Set AZURE_OPENAI_BASE_URL or AZURE_OPENAI_RESOURCE_NAME, or pass azureBaseUrl, azureResourceName, or model.baseUrl.`
- `Request was aborted`
- `An unknown error occurred`

### Controlled cloud example

Run inside a native Tokio runtime or a browser executor. This finite replacement
transport does not contact an external service.

```rust,no_run
use std::sync::Arc;
use maestro_models::{AzureOpenAIResponsesOptions, Context, Fetch, HttpResponse, Model, StreamOptions};
use maestro_models::providers::responses::azure_openai_responses::stream_azure_openai_responses;

async fn controlled_cloud(model: Model, context: Context) {
    let fetch: Fetch = Arc::new(|_| {
        let bytes = b"data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n\n".to_vec();
        Box::pin(std::future::ready(Ok(HttpResponse {
            status: 200, status_text: String::new(), headers: Default::default(),
            body: Box::pin(futures_util::stream::iter([Ok(bytes)])),
        })))
    });
    let stream = stream_azure_openai_responses(model, context, Some(AzureOpenAIResponsesOptions {
        azure_base_url: Some("https://controlled.invalid/v1".into()),
        common: StreamOptions { api_key: Some("controlled-key".into()), fetch: Some(fetch), ..Default::default() },
        ..Default::default()
    }));
    let message = stream.result().await;
    assert!(message.read().is_ok());
}
```

## Internal response conversion

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
selects whether a compound identity retains a separate item identity. A call
without an item identity replays without an `id` member. Replay retains the whole
item identity after the first `|`. Item identities starting with `fc_` are omitted for a different model only when
provider and API both match. Tool-result images follow the joined text when
supported, including image-only results; otherwise empty text uses
`(see attached image)`. `convert_responses_tools` preserves declaration order
and defaults `strict` to false; an explicit absent strict value sends null.

## Event reduction

`process_responses_stream` selects an event branch before decoding its members.
Unknown events and mismatched deltas leave unused fields unread. Missing or null
required containers and malformed consumed strings return native diagnostics. An
initial message's last content part is inspected only when a matching delta needs
its kind.

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
identity. Final-only calls are inserted before publishing their end. When creating
a call, an omitted item ID leaves the call identity alone; a present string ID
appends `|{id}`. A `call_id` containing `|` returns a native diagnostic because
it cannot be distinguished from the compound identity. Argument
completion delegates to the owning JSON helper; nonobject results become `{}`.
Argument completion replaces stored arguments, emitting a delta only when the
completed text adds a nonempty suffix to the previous scratch.

Completion publishes the final nonempty ID and model-priced usage before any
tier callbacks. Resolution runs only with pricing, outside message locks. Priced
usage is published before status selection, including when that selection fails.
Only a stop outcome is upgraded to tool use when a call exists. Reported input,
output, total and cached token counts default to zero when absent, null or zero;
numbers pass through, while other values return a native typed-read error.

Direct errors render `Error Code {code}: {message}`. Failed responses prefer a
truthy error, then a truthy incomplete reason, otherwise no-details text. Selected
SDK string fields, including deltas, call identity, error messages and final text
parts, reject nonstrings or missing required values with native typed-read errors.
A direct error code also accepts null, rendered as `null`; a function-call item ID
may be omitted, but a present value must be a string.
The read occurs only when its branch uses the value; already published content
survives a later malformed field. Raw truthiness still selects source defaults.

The reducer consumes to source EOF, propagating later failures even after
completion. EOF without a completed or incomplete event returns
`Response stream ended before a terminal event`. It publishes content updates
only; the caller owns final outcome events and stream termination. Standard and cloud
invocations use this reducer and supply the final outcome events.

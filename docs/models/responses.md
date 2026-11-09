# Response endpoints

The standard endpoint lives in `providers::responses::openai_responses`, and the
package root also exports `OpenAIResponsesOptions`. Conversion and event reduction
stay internal to the crate.

The raw and simple `_for` counterparts take a fourth `ProviderSpawner<E>` argument.
`OpenAIResponsesOptions<E>`, `AzureOpenAIResponsesOptions<E>` and
`SimpleStreamOptions<E>` keep hooks in that family, and `Local` admits caller-local
hooks and futures. These calls submit through the facility you supply. The default
three-argument calls keep their native runtime policy. The root simple `_for` exports
still settle setup errors as streams.

## Standard invocation

`stream_openai_responses` reports setup failures through its stream.
`stream_simple_openai_responses` rejects a missing provider key before it creates a
stream (the root export of the same name settles the error as an error stream; see the
[linkage guide](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/models/module-linkage.md)).
It then applies the shared simple budget and reasoning selection. Raw calls also fall
back to `OPENAI_API_KEY` after the explicit key and the provider key.

Cache retention follows the shared [options policy](../model-options.md#cache-retention).
Long retention is sent only when the model compatibility permits it. Disabling
`sendSessionIdHeader` leaves the generated `x-client-request-id` enabled for a nonempty
cache session. `none` suppresses the generated affinity and the prompt key; it does not
suppress headers you supply explicitly. A prompt key built from a session ID keeps at most
its first 64 Unicode characters, in Chat Completions, Responses, subscription-backed and
Azure requests alike. The session headers keep the full ID.

Reasoning options select the effort and summary for models that reason, and requested
reasoning includes encrypted content for replay. A summary alone selects the literal
`medium` effort, without consulting the model's effort mapping. Service tier options
distinguish omission, explicit null and named values. Pricing uses the echoed tier unless
it is absent or null, and then uses the requested tier. Flex scales cost by 0.5. Priority
uses 2.5 for exactly `gpt-5.5` and 2 for other models.

A payload hook may replace the request with any non-null JSON value. The replacement
uses the shared [compact JSON conversion](arguments.md#streamed-arguments), and a null
replacement fails before transport. Response hooks complete before Start and before the
streamed body is processed. If a payload hook disables streaming, the response body is
consumed before the response hook runs; a non-event response cannot feed the event
reducer. The transport, metadata and maximum retry-delay preferences are unused here. The
endpoint uses the shared
[HTTP transport](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/models/chat-completions.md#transport)
and the conversion and reduction described below. Streaming uses the
[shared event reader](chat-completions.md#stream-and-result).

### Controlled example

Run this inside a native Tokio runtime or a browser executor. The replacement
transport returns a finite response and contacts no external service.

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
`stream_azure_openai_responses` and `stream_simple_azure_openai_responses`, and the
package root also exports `AzureOpenAIResponsesOptions`. Output uses
`azure-openai-responses` even when the supplied descriptor names another API, and request
processing receives that descriptor unchanged. Raw setup failures are stream errors. The
simple invocation in this module rejects a missing provider key before it creates a stream
(the root export settles the error as an error stream), and it uses the shared simple
budgets and reasoning support.

Credentials select a nonempty explicit key, then the provider's environment key. Only raw
invocation then tries `AZURE_OPENAI_API_KEY`. Requests authenticate with `api-key`. Model
and caller headers override the defaults through the case-insensitive layering, except the
JSON `Content-Type`, which is applied last. Cloud invocation generates no affinity headers.

The base URL is the first of these that applies:

1. A trimmed nonblank `azure_base_url`.
2. `AZURE_OPENAI_BASE_URL`.
3. A nonempty `azure_resource_name` or `AZURE_OPENAI_RESOURCE_NAME`.
4. The descriptor base URL.

A selected invalid URL fails with no fallback. The resource shorthand builds
`https://{resource}.openai.azure.com/openai/v1`. Azure `OpenAI` and Cognitive Services host
roots and `/openai` paths normalize to `/openai/v1` and discard their query. Other paths
keep their non-version query pairs, including order, duplicates and escaped values.
`/responses` is appended to the pathname, and fragments are removed. No deployment segment
is generated.

The version is the first nonempty of `azure_api_version`, `AZURE_OPENAI_API_VERSION` and
`v1`. It replaces every query key that decodes exactly to `api-version`, and the inserted
value uses RFC3986 encoding, `%20` for spaces included. Resource names and versions are not
trimmed.

The deployment is the first nonempty of `azure_deployment_name`, the matching nonempty value
of `AZURE_OPENAI_DEPLOYMENT_NAME_MAP`, and the descriptor ID. Map entries are
comma-separated, and only the first two equals-separated fields count. Entry trimming comes
before the field-emptiness checks, and field trimming comes after them. A later valid entry
replaces an earlier one, even a whitespace-only value that becomes empty. A deployment
changes the `model` of the initial payload; it does not change the selected model's identity
or rates.

Before `on_payload`, the initial payload uses the supplied session ID, cut to 64 Unicode
characters, as `prompt_cache_key`, including an empty string, independently of retention.
The payload serializes `model`, `input`, `stream`, then `prompt_cache_key` when supplied,
then `store: false`; without a session ID, `store` follows `stream`. It omits retention,
service tier and metadata. Reasoning uses the shared effort mapping, without the standard
endpoint's account-provider exception, and a summary-only request selects the literal
`medium`. The hook can replace the entire payload, these selections and omissions
included. Usage uses the model rates without service-tier scaling. Hooks, HTTP retries,
timeouts and cancellation use the [transport](chat-completions.md#transport),
[JSON conversion](arguments.md#streamed-arguments) and
[response reduction](#event-reduction). The transport and maximum
retry-delay preferences are unused here.

### Cloud error messages

Failures that the cloud code writes use these messages. Transport, hook and reduction
errors keep the text of the operation that owns them:

- `Azure OpenAI API key is required. Set AZURE_OPENAI_API_KEY environment variable or pass it as an argument.`
- `No API key for provider: {provider}`
- `Invalid Azure OpenAI base URL: {baseUrl}`
- `Azure OpenAI base URL is required. Set AZURE_OPENAI_BASE_URL or AZURE_OPENAI_RESOURCE_NAME, or pass azureBaseUrl, azureResourceName, or model.baseUrl.`
- `Request was aborted`
- `An unknown error occurred`

### Controlled cloud example

Run this inside a native Tokio runtime or a browser executor. The finite replacement
transport contacts no external service.

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

Three operations convert history, convert tool declarations and reduce response event
text that is already framed. None opens a connection.

## Request items

`convert_responses_messages` uses the
[conversation projection](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/conversation-projection.md).
A nonempty system prompt is included by default, as `developer` for models that reason
and as `system` otherwise, or whenever the model's compatibility sets
`supportsDeveloperRole` to `false`. User content keeps its part order. An empty block
list produces no item, while empty text does.

Assistant text becomes a completed message. Version-one text signatures select the identity
and the recognized phase, and any other nonempty signature supplies the identity literally.
An identity longer than 64 UTF-16 units is hashed. A missing or empty identity uses the
number of preceding turns that produced items, and the later text blocks of one turn add
their position, so the identities within a turn differ. Thinking is replayed only when it
has a nonempty signature. Replay follows the
[JSON conversion rules](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/models/arguments.md),
so numeric spelling can change.

Tool identities are normalized for foreign history. The allowed-provider set decides
whether a compound identity keeps a separate item identity. A call without an item
identity replays without an `id` member, and replay keeps the whole item identity after
the first `|`. An item identity that starts with `fc_` is omitted for a different model
only when the provider and the API both match. Tool-result images follow the joined text
when the model supports them, image-only results included; otherwise empty text uses
`(see attached image)`. `convert_responses_tools` keeps the declaration order and defaults
`strict` to false. An explicitly absent strict value sends null.

## Event reduction

`process_responses_stream` selects an event branch before it decodes the members. Unknown
events and mismatched deltas leave unused fields unread. A missing or null required
container, or a malformed string it consumes, returns a native diagnostic. The last content
part of an initial message is inspected only when a matching delta needs its kind.

Text, thinking and tool-call updates share the supplied output handle. Every block that
opens keeps its index, even if a caller appends another block. Each open item is tracked by
the `output_index` its events carry, so items that finish out of order keep their own
reasoning signature, text and call arguments. Events without an index share one item.

- **Final items.** Final message items replace provisional text. Reasoning selects a
  nonempty final summary, then nonempty final content, and otherwise keeps the provisional
  thinking. A final reasoning, message or call item with no earlier added event opens its
  block and then finishes it.
- **Signatures.** Final text is stored before signature conversion, and a failed conversion
  emits no end event. Final reasoning keeps its whole item as compact JSON after a
  successful conversion. Encoded message signatures keep truthy phases; recognized phases
  are selected only during history conversion.
- **Calls.** An existing call uses its nonempty scratch arguments before the final
  arguments and ignores the final identity. A call that only finishes opens first, so its
  start precedes its end. When a call is created, an omitted item ID leaves the call
  identity alone, and a present string ID appends `|{id}`. A `call_id` that contains `|`
  returns a native diagnostic, because it cannot be told apart from the compound identity.
- **Arguments.** Argument completion delegates to the JSON helper, and a result that is not
  an object becomes `{}`. It replaces the stored arguments, and emits a delta only when the
  completed text adds a nonempty suffix to the previous scratch.

Completion publishes the final nonempty ID and the model-priced usage before any tier
callbacks. Resolution runs only with pricing, outside message locks. Priced usage is
published before the status is selected, even when that selection fails. Only a stop
outcome is upgraded to tool use, and only when a call exists. Reported input, output,
total and cached token counts default to zero when absent, null or zero. Numbers pass
through, and any other value returns a native typed-read error.

Direct errors render `Error Code {code}: {message}`. A failed response prefers a truthy
error, then a truthy incomplete reason, and otherwise uses no-details text. Selected SDK
string fields (deltas, call identity, error messages and final text parts) reject a
nonstring or a missing required value with a native typed-read error. A direct error code
also accepts null, rendered as `null`. A function-call item ID may be omitted, but a present
value must be a string. A read happens only when its branch uses the value, and content
already published survives a later malformed field. Raw truthiness still selects the default
values.

The reducer consumes the input to its end and propagates later failures even after
completion. An end of input without a completed or incomplete event returns
`Response stream ended before a terminal event`. The reducer publishes content updates only;
the caller owns the final outcome events and the end of the stream. Standard and cloud
invocations use this reducer and supply the final outcome events.

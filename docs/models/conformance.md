# Controlled model conformance

The controlled chat qualification cases inject finite HTTP responses through `Fetch`;
they do not contact a live provider. They exercise the generic `stream`,
`stream_simple`, `complete` and `complete_simple` interfaces. The gateway cases
use catalog routing, including the responses API for `gpt-5.1`.

## Qualified observations

- Absent or empty tool declarations are omitted without tool history; with
  assistant tool calls or tool results in history, they send an empty tools array.
  Raw and simple tool choices,
  strict-mode compatibility, catalog reasoning maps and tool-stream overrides
  are checked on the captured request.
- The first distinct nonempty reported model is retained separately from the
  requested model; the first nonempty response identifier is retained too.
- Ordered text, thinking and tool fragments are checked against the returned
  events and final message. Tool association uses index first, then identifier
  fallback; stream observations also compare their shared message handles.
- Completion token counts already include reasoning. Cache-write counts and costs
  are checked separately, including chunk usage and per-choice fallback.

These checks qualify conversion and accounting, not a server's cache-hit policy
or live billing. See [chat completions](chat-completions.md) for adapter rules and
[module linkage](module-linkage.md) for generic dispatch and option decoding.

## Run the cases

```sh
cargo test -p maestro-models --locked --test chat_empty_tools \
  --test chat_response_identity --test chat_tool_choices --test routed_cache_write
```

`routed_cache_write` supplies two controlled usage reports through
`complete_simple`, applying a test-owned ephemeral-cache marker.
Each invocation checks its own request and result.

## Inject a controlled response

This native Rust example uses the same catalog model and cache-write report as
that controlled completion. It needs `maestro-models`, `serde_json`, `tokio` with
`rt` and `time`, and `futures-util` with `std`. The synthetic credential is used
only by the injected transport.

```rust
use maestro_models::{Context, Fetch, HttpResponse, SimpleStreamOptions, StreamOptions};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fetch: Fetch = Arc::new(|_| {
        let wire = concat!(
            "data: {\"id\":\"controlled\",\"choices\":[{\"delta\":{\"content\":\"OK\"},",
            "\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":100,",
            "\"completion_tokens\":5,\"prompt_tokens_details\":{",
            "\"cached_tokens\":50,\"cache_write_tokens\":30}}}\n\n",
            "data: [DONE]\n\n"
        );
        Box::pin(std::future::ready(Ok(HttpResponse {
            status: 200,
            status_text: String::new(),
            headers: [("content-type".into(), "text/event-stream".into())].into(),
            body: Box::pin(futures_util::stream::iter([Ok(wire.as_bytes().to_vec())])),
        })))
    });
    let model = maestro_models::get_model("openrouter", "google/gemini-2.5-flash")
        .ok_or("missing catalog model")?;
    let context: Context = serde_json::from_value(serde_json::json!({
        "messages": [{"role": "user", "content": "Reply with exactly: OK", "timestamp": 1}]
    }))?;
    let options = SimpleStreamOptions {
        common: StreamOptions {
            api_key: Some("controlled-key".into()),
            fetch: Some(fetch),
            max_tokens: Some(32.0),
            temperature: Some(0.0),
            ..StreamOptions::default()
        },
        ..SimpleStreamOptions::default()
    };
    tokio::runtime::Builder::new_current_thread().enable_all().build()?.block_on(async {
        let message = maestro_models::complete_simple(model, context, Some(options)).await?;
        let message = message.read().map_err(|_| "message poisoned")?;
        assert_eq!(message.usage.cache_write, 30.0);
        assert_eq!(message.usage.cost.cache_write, 0.000_002_5);
        Ok::<_, Box<dyn std::error::Error>>(())
    })
}
```

## Controlled message conformance

The message qualification injects finite, named message events ending in
`message_stop`. Its 20 tests own 142 distinct inputs: 126 wire invocations,
15 fixed selector inputs and one catalog expansion. These responses qualify
local conversion, not remote acceptance, authentication validity, cache hits,
billing or reasoning quality. See [message protocol](messages.md) for the
adapter's request and reduction rules.

The cases exercise public `stream`, `stream_simple` and `complete` with real
catalog descriptors. Captured requests, ordered events and completed messages
are compared with recorded expectations. The catalog expansion checks candidate
order and one selected probe per provider; fixed inputs distinguish priority,
cost arithmetic, stable ties, locale collation and filtering before selection.

The message cases cover:

- Eager tool-input defaults, legacy compatibility and requests without tools;
  configured and supported forced probes use the selected catalog routes.
- Cache-marker placement with none, short and long retention, including final
  user text, image and tool-result blocks and an assistant-last history.
  Four eager/long compatibility pairs check the switches independently.
- Simple calls without reasoning disable thinking on `claude-sonnet-4-5`,
  `claude-opus-4-6` and `claude-opus-4-7`; raw calls distinguish absent, false
  and true enablement. Effort mappings, display modes, signed thinking fragments
  and exact arithmetic text have separate controlled observations.
- Subscription tool-name conversion, first-declared inbound spelling and
  unchanged caller context; ordinary keys and unrelated names provide contrasts.
  Tool-end observations inspect the same invocation's shared message handle.
- Account authentication and headers, user/assistant initiator and image input;
  interleaved beta controls are crossed with thinking enablement and adaptive
  model selection, with a separate legacy tool-streaming case.

Run these cases without provider credentials:

```sh
cargo test -p maestro-models --locked --test message_eager_input \
  --test message_long_retention --test message_thinking_disabled \
  --test message_adaptive_thinking --test account_tool_names \
  --test account_message_routing
```

### Inject a completed message

This native example uses the same dependencies as the chat example above.
The injected transport returns text and a terminal message event; the synthetic
credential never goes to a provider.

```rust
use maestro_models::{Context, Fetch, HttpResponse, SimpleStreamOptions, StreamOptions};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fetch: Fetch = Arc::new(|_| {
        let events = [
            serde_json::json!({"type":"message_start","message":{
                "id":"controlled","type":"message","role":"assistant",
                "model":"claude-opus-4-7","content":[],"stop_reason":null,
                "stop_sequence":null,"usage":{"input_tokens":3,"output_tokens":0}}}),
            serde_json::json!({"type":"content_block_start","index":0,
                "content_block":{"type":"text","text":""}}),
            serde_json::json!({"type":"content_block_delta","index":0,
                "delta":{"type":"text_delta","text":"OK"}}),
            serde_json::json!({"type":"content_block_stop","index":0}),
            serde_json::json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},
                "usage":{"output_tokens":1}}),
            serde_json::json!({"type":"message_stop"}),
        ];
        let wire: String = events.iter().map(|event| {
            format!("event: {}\ndata: {event}\n\n", event["type"].as_str().unwrap_or(""))
        }).collect();
        Box::pin(std::future::ready(Ok(HttpResponse {
            status: 200,
            status_text: String::new(),
            headers: [("content-type".into(), "text/event-stream".into())].into(),
            body: Box::pin(futures_util::stream::iter([Ok(wire.into_bytes())])),
        })))
    });
    let model = maestro_models::get_model("anthropic", "claude-opus-4-7")
        .ok_or("missing catalog model")?;
    let context: Context = serde_json::from_value(serde_json::json!({
        "messages": [{"role":"user","content":"Reply with exactly: OK","timestamp":1}]
    }))?;
    let options = SimpleStreamOptions {
        common: StreamOptions {
            api_key: Some("controlled-key".into()), fetch: Some(fetch),
            max_tokens: Some(32.0), ..StreamOptions::default()
        },
        ..SimpleStreamOptions::default()
    };
    tokio::runtime::Builder::new_current_thread().enable_all().build()?.block_on(async {
        let message = maestro_models::complete_simple(model, context, Some(options)).await?;
        let message = message.read().map_err(|_| "message poisoned")?;
        assert_eq!(message.stop_reason, maestro_models::StopReason::Stop);
        assert!(matches!(&message.content[0], maestro_models::AssistantContent::Text(text)
            if text.text == "OK"));
        Ok::<_, Box<dyn std::error::Error>>(())
    })
}
```

## Controlled response lifetimes and handoffs

The response qualification cases use catalog models with injected, finite server
responses through public streaming and completion operations. They compare
payload callbacks, application-owned request headers and bodies, terminal records
and recorded stream events. Continuations use actual returned assistants and
returned tool identifiers. Generated histories are compared with the recorded
inputs before dispatch. Payload, request, terminal-record and recorded-event
comparison failures include the model identity, input history, captured payloads
and actual result, without invocation credentials.

The cases exercise response identity on successful and failed turns, compatible
and foreign reasoning replay, cache/session request affinity, native tool-result
images, interleaved reasoning and pre-cancelled or partially cancelled streams.
Partial cancellation waits for a public text delta and a pending body read;
terminal observations follow draining and body release. Ordered handoff histories
keep the separate regional routes and tool occurrences, including missing results.
See [response protocols](responses.md),
[response sessions](response-sessions.md) and
[history projection](../conversation-projection.md) for the owning behavior.

These controlled responses establish local request and decoding observations,
not live authentication acceptance, cache hits or remote answer quality.

```sh
cargo test -p maestro-models --locked --test response_identity \
  --test response_reasoning_replay --test response_affinity \
  --test response_session_affinity --test response_tool_images \
  --test interleaved_reasoning --test invocation_cancellation \
  --test history_handoffs --test handoff_tool_ids
```

## Controlled reasoning and Unicode histories

The reasoning and text cases own 185 recorded inputs: 164 finite wire invocations,
one ordered cloud catalog query and 20 raw UTF-16 sanitizer inputs. Fifteen tests
compare these local observations through public operations. Wire cases use the
shipped adapters; catalog presence does not establish remote availability. The
missing-result and Unicode input groups each retain 23 model/account routes, including separate ordinary and
account-authenticated message cases.

The cases compare native conversation reasoning controls and their omission when
reasoning is absent, four shipped thinking-disable requests, and nested schema
keywords in callback and wire payloads. See
[reasoning conversations](reasoning-conversations.md) for the owning controls.
Missing-result continuations use actual returned assistants, with separate
next-tool, history-boundary and failed-assistant inputs. See
[history projection](../conversation-projection.md) for repair behavior.

Text observations retain emoji tool results, multiline comments and distinct
role text. Byte-body cases compare ordered stream events and terminal records
for one-byte chunks and an invalid byte inside a text delta, including the
response protocols' later authoritative text replacement. Raw-unit cases call
the existing sanitizer. Decoder rules belong to [chat completions](chat-completions.md),
[message protocol](messages.md) and [response protocols](responses.md).
These finite fixtures qualify local conversion, not remote reasoning quality,
authentication validity or server schema acceptance.

```sh
cargo test -p maestro-models --locked --test cloud_model_routes \
  --test content_thinking_disabled --test conversation_reasoning \
  --test conversation_tool_schema --test missing_tool_results \
  --test provider_unicode
```

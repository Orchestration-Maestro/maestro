# Controlled chat conformance

The controlled qualification cases inject finite HTTP responses through `Fetch`;
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

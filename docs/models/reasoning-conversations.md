# Reasoning conversation requests

`MistralOptions` holds common request settings, tool choice, prompt mode and
reasoning effort. The options are available from `maestro_models` and
`maestro_models::providers::reasoning::mistral`, together with `stream_mistral`
and `stream_simple_mistral`. These operations expose cumulative updates from
independently owned work: dropping the reader does not cancel the producer.
Native calls need a running Tokio runtime.
Missing credentials produce a terminal error for raw calls and an immediate
error for simple calls. Text, thinking and tool updates retain the same message
handle through the terminal result.

```rust
use maestro_models::{MistralOptions, MistralPromptMode, MistralReasoningEffort,
    MistralToolChoice};

let options = MistralOptions {
    tool_choice: Some(MistralToolChoice::Function { name: "lookup".into() }),
    prompt_mode: Some(MistralPromptMode::Reasoning),
    reasoning_effort: Some(MistralReasoningEffort::High),
    ..Default::default()
};
assert_eq!(options.prompt_mode, Some(MistralPromptMode::Reasoning));
```

This controlled transport uses no network or secret credential:

```rust
use std::sync::Arc;
use maestro_models::{Context, Fetch, HttpResponse, MistralOptions, StreamOptions,
    get_model, stream_mistral};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let fetch: Fetch = Arc::new(|_| Box::pin(async {
    Ok(HttpResponse {
        status: 200,
        status_text: String::new(),
        headers: [("content-type".into(), "text/event-stream".into())].into(),
        body: Box::pin(futures_util::stream::iter([
            Ok(b"data: [DONE]\n\n".to_vec())
        ])),
    })
}));
let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
runtime.block_on(async {
    let model = get_model("mistral", "mistral-small-2603").ok_or("missing model")?;
    let stream = stream_mistral(model, Context {
        system_prompt: None, messages: vec![], tools: None,
    }, Some(MistralOptions {
        common: StreamOptions {
            api_key: Some("fixture-key".into()), fetch: Some(fetch),
            ..Default::default()
        }, ..Default::default()
    }));
    let result = stream.result().await;
    assert!(result.read().map_err(|_| "message lock poisoned")?.content.is_empty());
    Ok::<(), Box<dyn std::error::Error>>(())
})?;
# Ok(())
# }
```

The private request encoder selects recognized fields from object-shaped records,
adds their declared defaults and emits transport field names. Optional null is
retained only where the field admits it. Unknown record members are omitted after
decoding; dictionaries use the [shared JSON representation](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/records.md).
Invalid selected fields return `Input validation failed: `
followed by the native decoder's explanation.

The encoder has no transport, retry or cancellation policy. See the
[shared model records](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/records.md) for the owning JSON representation.

Private preparation projects history, selects model-specific simple reasoning
controls and encodes payload-hook replacements. Raw controls remain explicit.
Authored model and option headers merge with exact letter case before HTTP case
variants combine. Credential headers are validated before authored overrides, so
an override cannot conceal an invalid credential header. This provider does not
call the response hook.

Private sending makes one attempt. Without a caller cancellation signal, one
30-second deadline covers request setup and body reads; a supplied signal disables
that deadline. Admission requires status 200 with `text/event-stream`, allowing
case differences and parameters. A 422 JSON response is syntax-checked without
selecting error-record fields. Nonempty trimmed HTTP error bodies retain at most
4,000 Unicode scalars before a truncation suffix; native causes and empty-body
fallback messages are not subject to that limit.

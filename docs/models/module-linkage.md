# Bundled protocol linkage

`maestro-models` links its bundled protocols statically. The registry already holds
them the first time anything reads it, so callers register nothing before invoking
a bundled model. A supplied descriptor needs no catalog membership; the registry
looks up only its `api` identifier.

## Registered protocols

The registry starts with these protocols, in this order:

1. `anthropic-messages`
2. `openai-completions`
3. `mistral-conversations`
4. `openai-responses`
5. `azure-openai-responses`

The simulated provider is never registered implicitly, and no placeholder stands in
for a protocol whose adapter this crate does not contain.

| Operation | Effect |
|---|---|
| `register_built_in_api_providers` | Replaces the entry of each bundled protocol in place, keeping its position and leaving it without a source owner. Custom entries are untouched. |
| `reset_api_providers` | Removes every entry, then registers the bundled protocols. Retained handles to removed entries keep working. |
| `clear_api_providers` | Leaves the registry empty. Neither a later lookup nor an invocation registers the bundled protocols again; typed root calls below do not use the registry. |

## Root entry points

`stream_anthropic`, `stream_openai_completions`, `stream_mistral`,
`stream_openai_responses` and `stream_azure_openai_responses` take their typed
options and never consult the registry. Each `stream_simple_*` function of the
same families takes `SimpleStreamOptions` and always returns a stream. A missing key,
which the functions of the same names in the provider modules report as `Err`, ends
it with an error update carrying the adapter's text, an empty message with the
descriptor's identity and zero usage, and the error stop reason even when the
signal was already aborted. Every other failure follows the adapter's own stream
path; see each provider page.

`stream`, `stream_simple`, `complete` and `complete_simple` resolve the registry.
An unregistered `api` returns `No API provider registered for api: {api}`. A
registered custom callback keeps returning its own setup error immediately (inside
the future for `complete`). A bundled callback never does: its setup failures are
settled error streams.

## Raw extras

`ProviderStreamOptions.common` reaches the adapter unchanged. A bundled raw
callback reads these optional members of `ProviderStreamOptions.extra`, ignores
all others and ends the stream with the decoder's error text when a listed member
has the wrong shape. Only the two members marked nullable accept `null`.

| Protocol | Members |
|---|---|
| `anthropic-messages` | `thinkingEnabled`, `thinkingBudgetTokens`, `effort` (`low`, `medium`, `high`, `xhigh`, `max`), `thinkingDisplay` (`summarized`, `omitted`), `interleavedThinking`, `toolChoice` (`auto`, `any`, `none` or `{"type":"tool","name":…}`) |
| `openai-completions` | `reasoningEffort` (`minimal` to `xhigh`), `toolChoice` (`auto`, `none`, `required` or a named function) |
| `mistral-conversations` | `reasoningEffort` (`none`, `high`), `promptMode` (`reasoning`), `toolChoice` (`auto`, `none`, `any`, `required` or a named function) |
| `openai-responses` | `reasoningEffort`, `reasoningSummary` (`auto`, `detailed`, `concise`, nullable), `serviceTier` (`auto`, `default`, `flex`, `scale`, `priority`, nullable) |
| `azure-openai-responses` | `reasoningEffort`, `reasoningSummary`, `azureApiVersion`, `azureResourceName`, `azureBaseUrl`, `azureDeploymentName` |

A `null` summary and a missing summary both request no explicit summary. A
`null` service tier is sent as `null`; a missing one is omitted.

## Objects that are not JSON

`ProviderObjects` carries typed values next to the JSON extras, at most one per
concrete type. `insert` replaces the value of its type, `get` borrows it, and
`Clone` shares the stored values while giving each copy its own entries. The
`anthropic-messages` callback takes an `AnthropicClient` from it, so a caller can supply its
own transport through generic dispatch. Debug output shows only the entry count.

## Example

```rust
use maestro_models::{
    Context, Model, ProviderObjects, ProviderStreamOptions, StopReason, get_api_providers,
    reset_api_providers, stream, stream_simple_openai_completions,
};
use std::{future::Future, task::{Context as PollContext, Poll, Waker}};

reset_api_providers();
let apis: Vec<String> = get_api_providers().into_iter().map(|provider| provider.api).collect();
assert_eq!(apis, [
    "anthropic-messages", "openai-completions", "mistral-conversations",
    "openai-responses", "azure-openai-responses",
]);

let model: Model = serde_json::from_value(serde_json::json!({
    "id": "controlled", "name": "Controlled model", "api": "openai-completions",
    "provider": "fixture", "baseUrl": "https://fixture.invalid",
    "reasoning": false, "input": ["text"], "contextWindow": 100, "maxTokens": 10,
    "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0}
}))?;
let context = Context { system_prompt: None, messages: vec![], tools: None };
let mut poll = PollContext::from_waker(Waker::noop());

// No key for provider `fixture`: the stream settles with an error.
let keyless = stream_simple_openai_completions(model.clone(), context.clone(), None);
let mut result = std::pin::pin!(keyless.result());
let Poll::Ready(message) = result.as_mut().poll(&mut poll) else { return Err("settled".into()) };
assert_eq!(message.read().map_err(|_| "poisoned")?.stop_reason, StopReason::Error);

// A known extra of the wrong shape settles before any request is built.
let extra = serde_json::json!({"reasoningEffort": 5});
let options = ProviderStreamOptions { extra: serde_json::from_value(extra)?, ..Default::default() };
let rejected = stream(model, context, Some(options))?;
let mut result = std::pin::pin!(rejected.result());
let Poll::Ready(message) = result.as_mut().poll(&mut poll) else { return Err("settled".into()) };
assert_eq!(message.read().map_err(|_| "poisoned")?.stop_reason, StopReason::Error);

let mut objects = ProviderObjects::default();
objects.insert(7_u32);
assert_eq!(objects.get::<u32>(), Some(&7));
# Ok::<(), Box<dyn std::error::Error>>(())
```

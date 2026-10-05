# Thinking and output options

The application supplies `StreamOptions`; `Models` resolves them against captured
registered `RequestCapabilities`. `resolve_options` does not dispatch, consume
scripts, sample the clock or emit events. `stream` resolves once before one
adapter invocation; `complete` drains that same stream. A caller model copy cannot
override registered capabilities. Direct adapters accept authorized `ProviderOptions` with resolved choices, not
unresolved choices. `EffectiveOptions` is the pure resolution result; dispatch
adds supplied authentication and effective headers. Assistant records do not acquire request effort fields.

## Thinking declarations

Default options request Off, not an application startup preference. Nonreasoning
models always resolve Off without effort or budget. On reasoning models, absent
map entries support Off through High; Xhigh requires an explicit `Some` mapping.
A present `None` disables a level. Unsupported levels scan upward first, otherwise
downward. An entirely disabled map falls back to Off without any thinking field.
Effort mode uses mapped strings verbatim, even a mapped Off; unmapped non-Off
uses its lowercase name and unmapped Off emits no effort. TokenBudget mappings
only declare support, never numeric budgets. Identity and protocol labels do not
imply capabilities.

```rust
use maestro_models::*;
use std::sync::Arc;

let mut model = Model {
    identity: ModelIdentity {
        provider: "local".into(), model: "example".into(), operation: "chat".into(),
    },
    protocol: "declared-chat".into(),
    capabilities: RequestCapabilities { reasoning: true, ..Default::default() },
    headers: Default::default(),
    input: vec!["text".into()],
};
model.capabilities.thinking_level_map.insert(Minimal, None);
# use ThinkingLevel::{Off, Minimal, Low, Medium, High, Xhigh};
model.capabilities.thinking_level_map.insert(High, None);
let mut models = Models::new(Arc::new(|| 73));
models.register(model.clone(), Arc::new(ScriptedProvider::new(vec![])))?;
let resolve = |thinking| models.resolve_options(&model, StreamOptions {
    thinking, ..Default::default()
});
assert_eq!(resolve(Minimal)?.thinking, Low); // Upward before downward.
assert_eq!(resolve(High)?.thinking, Medium); // Xhigh is not enabled.
model.capabilities.thinking_level_map.insert(Xhigh, Some("extra".into()));
let mut enabled = Models::new(Arc::new(|| 73));
enabled.register(model.clone(), Arc::new(ScriptedProvider::new(vec![])))?;
let result = enabled.resolve_options(&model, StreamOptions {
    thinking: Xhigh, ..Default::default()
})?;
assert_eq!(result.requested_thinking, Xhigh);
assert_eq!(result.thinking, Xhigh);
assert_eq!(result.effort.as_deref(), Some("extra"));
# Ok::<(), Failure>(())
```

## Base output and shared ceilings

An explicit base allowance survives unchanged in simple/Effort requests, including
zero, values above 32,000 or the model ceiling and `u64::MAX`. Omitted output uses
`min(positive ceiling, 32_000)`; nonpositive metadata leaves it unspecified. Zero
means unknown metadata, not a catalog fallback. No context estimation or safety
allowance is applied.

Enabled TokenBudget reasoning uses Minimal = 1,024, Low = 2,048, Medium = 8,192,
High = 16,384; explicitly supported Xhigh aliases High. Output becomes
`min(base + budget, ceiling)` without overflow. When output is **at or below**
budget, budget becomes `output.saturating_sub(1_024)`; otherwise it is unchanged.
Off bypasses this adjustment. Enabled TokenBudget with a nonpositive ceiling
returns `Failure::UnsupportedOperation` before dispatch, even with explicit output.
Effort and Off retain normal unspecified-output behavior. Cancellation is checked
before lookup, after the capability callback and finally before dispatch.

```rust
use maestro_models::*;
use std::sync::Arc;
use ThinkingLevel::Medium;

fn resolve(ceiling: i64, mode: ThinkingMode, output: Option<u64>) -> Result<EffectiveOptions, Failure> {
    let model = Model {
        identity: ModelIdentity {
            provider: "local".into(), model: "example".into(), operation: "chat".into(),
        },
        protocol: "declared-chat".into(),
        capabilities: RequestCapabilities {
            reasoning: true, output_limit: ceiling, thinking_mode: mode,
            ..Default::default()
        },
        headers: Default::default(),
    input: vec!["text".into()],
    };
    let mut models = Models::new(Arc::new(|| 73));
    models.register(model.clone(), Arc::new(ScriptedProvider::new(vec![])))?;
    models.resolve_options(&model, StreamOptions {
        thinking: Medium, output_limit: output, ..Default::default()
    })
}
for (ceiling, expected) in [(31_999, 31_999), (32_000, 32_000), (32_001, 32_000)] {
    assert_eq!(resolve(ceiling, ThinkingMode::Effort, None)?.output_limit, Some(expected));
}
assert_eq!(resolve(32_000, ThinkingMode::Effort, Some(40_000))?.output_limit, Some(40_000));
assert_eq!(resolve(0, ThinkingMode::Effort, None)?.output_limit, None);
for (ceiling, budget) in [(8_191, 7_167), (8_192, 7_168), (8_193, 8_192), (1_024, 0)] {
    let result = resolve(ceiling, ThinkingMode::TokenBudget, Some(10_000))?;
    assert_eq!(result.output_limit, Some(ceiling as u64));
    assert_eq!(result.thinking_budget, Some(budget));
}
let equality = resolve(100_000, ThinkingMode::TokenBudget, Some(0))?;
assert_eq!(equality.output_limit, Some(8_192));
assert_eq!(equality.thinking_budget, Some(7_168));
assert!(matches!(resolve(0, ThinkingMode::TokenBudget, None), Err(Failure::UnsupportedOperation)));
# Ok::<(), Failure>(())
```

## Supported preferences and request factories

Temperature and session affinity require their declared booleans;
transport/cache preferences must belong to their declared sets. Supported values
are preserved exactly, including zero temperature. Unsupported scalars are absent. Absent values stay absent:
there is no default temperature, ambient environment overlay, automatic transport
or cache lifetime selection. Preference identifiers are data, not implementations.
Headers are not capability-filtered preferences: provider/model headers form the
base and explicit request headers overlay them case-insensitively, through the
existing request-authentication validation path. Actual wire encoding belongs to
the connection adapter; these declarations describe its effective inputs.

```rust
use maestro_models::*;
use std::sync::Arc;
use ThinkingLevel::Medium;

async fn inspect_request() -> Result<(), Failure> {
    let model = Model {
        identity: ModelIdentity {
            provider: "local".into(), model: "example".into(), operation: "chat".into(),
        },
        protocol: "declared-chat".into(),
        capabilities: RequestCapabilities {
            reasoning: true, output_limit: 32_001, temperature: true,
            transports: ["sse".into()].into(),
            ..Default::default()
        },
        headers: Default::default(),
    input: vec!["text".into()],
    };
    let fake = Arc::new(ScriptedProvider::new(vec![Script::Factory(Box::new(|call| {
        Box::pin(async move {
            assert_eq!(call.options.requested_thinking, Medium);
            assert_eq!(call.options.effort.as_deref(), Some("medium"));
            assert_eq!(call.options.output_limit, Some(32_000));
            assert_eq!(call.options.temperature, Some(0.0));
            assert_eq!(call.options.transport.as_deref(), Some("sse"));
            assert_eq!(call.options.cache_preference, None);
            assert_eq!(call.options.headers["x-example"], "exact");
            Ok(vec![ScriptStep::Update(ProviderUpdate::Done { reason: StopReason::Stop })])
        })
    }))]));
    let options = StreamOptions {
        auth: Some(RequestAuth::ConfiguredWithoutSecret { source: None }),
        thinking: Medium, temperature: Some(0.0), transport: Some("sse".into()),
        cache_preference: Some("long".into()),
        headers: [("X-Example".into(), "exact".into())].into(),
        ..Default::default()
    };
    let mut models = Models::new(Arc::new(|| 73));
    models.register(model.clone(), fake.clone())?;
    let effective = models.resolve_options(&model, options.clone())?;
    assert_eq!(effective.cache_preference, None);
    assert_eq!(fake.pending(), 1); // Resolution did not consume the factory.
    assert!(fake.calls().is_empty());
    let result = models.complete(model, Context { system_prompt: None, messages: vec![], tools: vec![] }, options).await;
    assert_eq!(result.stop_reason, Some(StopReason::Stop));
    assert_eq!(fake.calls().len(), 1);
    Ok(())
}
```

Owned options, registered metadata and observations remain independent of later
caller or observation-clone mutation. Cancellation alone intentionally shares its
signal through requested/effective options and the normalizer. Separately
defaulted requests are independent. A blocked request can be cancelled without
opening its controlled gate, retrying or losing valid partial content.

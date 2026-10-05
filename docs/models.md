# Model streaming and scripted responses

`maestro-models` provides explicitly registered chat access through replaceable
adapters. The registry owns provider/model/operation dispatch; adapters supply
indexed updates and the normalizer owns block, failure and cancellation rules.
Requests require explicit authentication or an injected selected-provider resolver.
See [request authentication](request-authentication.md) for ownership, headers
and secret-free local access. Adapters and scripted factories receive effective options resolved from registered
capabilities; see [Thinking and output options](model-options.md).
The crate has no internal workspace dependencies. Its JSON dependency parses
completed tool arguments strictly; it requires no asynchronous runtime.

Mixed conversation records keep current prompt/tools separate from history.
The registry supplies one owned, selected-model projection before adapter
dispatch. See [conversation projection](conversation-projection.md) for replay,
image omissions, paired tool results and pure offline argument validation.
See [local model catalogs](local-model-catalogs.md) for offline lookup, metadata
validation, reversible overrides and captured-request replacement semantics.

## Explicit indexed streaming

This example compiles without credentials or network access. An application
executor runs the async body; tests use a private standard-library executor.

```rust
use maestro_models::*;
use std::sync::Arc;

async fn scripted_text() -> Result<(), Failure> {
    let model = Model::custom(
        ModelIdentity {
            provider: "scripted:example".into(), model: "greeting/text".into(),
            operation: "chat".into(),
        },
        "scripted/chat".into(),
        "local:endpoint".into(),
    );
    let context = Context {
        system_prompt: Some("Reply with a greeting".into()),
        messages: vec![Message::User(UserMessage { content: vec![InputContent::Text(TextContent { text: "hello".into(), replay_metadata: None })], timestamp: 17 })],
        tools: vec![],
    };
    let response = || Script::Steps(vec![
        ScriptStep::Update(ProviderUpdate::TextStart { content_index: 0 }),
        ScriptStep::Update(ProviderUpdate::TextDelta {
            content_index: 0, delta: "hel".into(),
        }),
        ScriptStep::Update(ProviderUpdate::TextDelta {
            content_index: 0, delta: "lo".into(),
        }),
        ScriptStep::Update(ProviderUpdate::TextEnd { content_index: 0, replay_metadata: None }),
        ScriptStep::Update(ProviderUpdate::Usage {
            usage: Usage { input: 11, output: 7, total_tokens: 18, ..Usage::default() },
        }),
        ScriptStep::Update(ProviderUpdate::Done { reason: StopReason::Stop }),
    ]);
    let fake = Arc::new(ScriptedProvider::new(vec![response(), response()]));
    let mut models = Models::new(Arc::new(|| 73));
    models.register(model.clone(), fake.clone())?;
    let options = StreamOptions {
        auth: Some(RequestAuth::ConfiguredWithoutSecret { source: None }),
        ..Default::default()
    };
    let mut stream = models.stream(model.clone(), context.clone(), options.clone());
    let mut terminal = None;
    while let Some(event) = stream.next().await {
        match event {
            ModelEvent::TextDelta { delta, .. } => assert!(delta == "hel" || delta == "lo"),
            ModelEvent::Done { message, .. } => terminal = Some(message),
            ModelEvent::Error { error, .. } => return Err(error.failure.unwrap()),
            _ => {},
        }
    }
    let completed = models.complete(model.clone(), context.clone(), options.clone()).await;
    assert_eq!(terminal.as_ref(), Some(&completed));
    assert_eq!(completed.content[0], AssistantContent::Text(TextContent { text: "hello".into(), replay_metadata: None }));
    assert_eq!(completed.timestamp, 73);
    assert_eq!(fake.calls()[0].model, model);
    assert_eq!(fake.calls()[0].context, context);
    assert_eq!(fake.calls().len(), 2);
    assert_eq!(fake.pending(), 0);
    Ok(())
}
```

Success emits one `Start`, balanced text/thinking/tool-call starts and ends, and
one `Done` with `Stop`, `Length` or `ToolUse`. Empty success has exactly Start/Done
and invents no block. New blocks append at the next index; any open blocks may
interleave. Empty deltas are retained exactly. Readable thinking starts empty
and retains its optional signature. TextEnd attaches optional opaque replay
metadata to its block before the closing snapshot. Redacted thinking introduces an opaque block
with ThinkingStart/ThinkingEnd and no delta; ThinkingEnd's readable content is
empty. Do not display opaque data as reasoning.

Tool-call deltas carry JSON fragments. `ToolCall::arguments()` returns `None`
until ToolCallEnd accepts a strict JSON object, including explicitly supplied
`{}`. Truncated JSON, non-object values and invalid escapes fail; no repair or
placeholder object is invented. Availability is not execution authorization:
require successful terminal completion and apply tool validation/policy before
execution. An unfinished failed or aborted call retains unavailable arguments.

Every event owns cumulative content, nested tool arguments, usage and identity.
Retaining or modifying an owned clone cannot rewrite other events. Requested
provider/protocol/model and the injected clock's one invocation sample remain
unchanged. Optional actual response model/ID remain separate. Explicit Usage
updates replace non-overlapping counters; the normalizer derives totals and flat
catalog-rate estimates. Initial zeros remain unreported, not measured zero consumption.

`complete` drains exactly one invocation of the same stream and returns its
terminal record. Registering another implementation of `Provider` and
`ProviderStream` does not require caller edits. Synchronous provider setup must
not block; asynchronous work belongs in the source.

## Queued request factories

```rust
use maestro_models::*;
use std::sync::Arc;

async fn request_factory(model: Model, context: Context) -> Result<(), Failure> {
    let fake = Arc::new(ScriptedProvider::new(vec![Script::Factory(Box::new(|call| {
        Box::pin(async move {
            assert_eq!(call.call_index, 1);
            let Message::User(user) = &call.context.messages[0] else { panic!("expected user") };
            let InputContent::Text(block) = &user.content[0] else { panic!("expected text") };
            let text = block.text.clone();
            Ok(vec![
                ScriptStep::Update(ProviderUpdate::TextStart { content_index: 0 }),
                ScriptStep::Update(ProviderUpdate::TextDelta { content_index: 0, delta: text }),
                ScriptStep::Update(ProviderUpdate::TextEnd { content_index: 0, replay_metadata: None }),
                ScriptStep::Update(ProviderUpdate::Done { reason: StopReason::Stop }),
            ])
        })
    }))]));
    let mut models = Models::new(Arc::new(|| 73));
    models.register(model.clone(), fake.clone())?;
    let options = StreamOptions {
        auth: Some(RequestAuth::ConfiguredWithoutSecret { source: None }),
        ..Default::default()
    };
    let result = models.complete(model, context, options).await;
    assert_eq!(result.stop_reason, Some(StopReason::Stop));
    assert_eq!(fake.pending(), 0);
    assert_eq!(fake.calls()[0].call_index, 1);
    Ok(())
}
```

Each `Script` is one queued request response, not a deferred job. FIFO dispatch
atomically records the owned request and removes at most one script. `calls()`
clones request data; options deliberately retain shared cancellation semantics.
`pending()` counts queued Script values, excluding the dispatched response.
Exhaustion records a call and returns ScriptExhausted, never replaying a response.
SetupFailure supplies a typed synchronous error. A Factory runs once, lazily on
the first asynchronous source read, before Start; it may inspect observations
without a queue lock being held. Its typed failure uses the normal pre-start
error contract. `ScriptStep::Wait` accepts a caller-controlled future: no sleeps,
random pacing or inferred token usage. Active wait/factory futures belong to the
source, so dropping and recreating a pending read does not consume a wait or
restart a factory.

## Cancellation and failures

```rust
use maestro_models::*;

async fn cancelled_request(models: &Models, model: Model, context: Context) {
    let options = StreamOptions {
        auth: Some(RequestAuth::ConfiguredWithoutSecret { source: None }),
        ..Default::default()
    };
    let signal = options.cancellation.clone();
    signal.cancel();
    let result = models.complete(model, context, options).await;
    assert_eq!(result.stop_reason, Some(StopReason::Aborted));
    assert_eq!(result.failure, Some(Failure::Cancelled));
    signal.cancel(); // Repeated cancellation is harmless.
    signal.cancelled().await; // Also completes when cancelled before polling.
}
```

Clones share one signal; separately created/defaulted requests remain independent.
Cancellation before dispatch consumes no script and records no provider call.
While a source read is blocked, cancellation wakes its waiter without opening a
fixture gate, drops the outstanding read/source/factory future and returns one
Error with Aborted/Cancelled, retaining valid content/usage/identity. Dropped
cancellation waiters unregister. Cancellation wins observable source-readiness
and pending block-end/terminal ties until terminal delivery; after delivered
termination it cannot replace the result. No retry occurs. This stops local
work/waiting, not remote effects already performed.

Unknown identities, unsupported operations/protocols and duplicate registrations
remain distinct typed failures. Setup, factory and source failures become Error
rather than escaped exceptions or Done. Error outcomes use Error; cancellation
uses Aborted. Fixed useful `Failure` display text never interpolates raw parser
errors, identifiers, request contents or secrets. Started failures preserve valid
partial state. Illegal indices/families, closed-block updates, invalid tool JSON,
non-success Done reasons and Done with open blocks are MalformedStream. EOF
without a terminal update is IncompleteStream, including an empty source or
closed content lacking Done. Metadata-only updates require no invented content
event. After one terminal delivery the source is dropped and `next()` permanently
returns None; later source updates are never polled.

## Flat reported usage and cost estimates

Each attempt starts with `Usage::default()`: zero counters/costs, `reported = false`
and `cost.priced = false`, even with catalog rates. Content length and thinking
never fabricate a measurement. Each explicit report replaces the previous four
categories, including explicit all-zero reports (`reported = true`).

Adapters supply non-overlapping categories: input excludes both cache categories,
cache reads exclude current cache writes, and output already includes reasoning.
Raw inclusive-prompt/combined-cache subtraction belongs to the adapter. The shared
normalizer ignores adapter totals, flags and monetary values, derives the category
sum and applies captured registered rates in USD per million tokens. Invocation
rate metadata and actual response identity do not change those prices.

```rust
use maestro_models::*;
use std::sync::Arc;

async fn flat_accounting() -> Result<(), Failure> {
    let model = Model {
        identity: ModelIdentity {
            provider: "scripted:accounting".into(), model: "flat/chat".into(),
            operation: "chat".into(),
        },
        protocol: "scripted/chat".into(),
        capabilities: RequestCapabilities::default(),
        input: vec!["text".into()],
        rates: Some(TokenRates { input: 2.0, output: 8.0, cache_read: 1.0, cache_write: 4.0 }),
        headers: Default::default(),
    };
    let fake = Arc::new(ScriptedProvider::new(vec![Script::Steps(vec![
        ScriptStep::Update(ProviderUpdate::TextStart { content_index: 0 }),
        ScriptStep::Update(ProviderUpdate::TextEnd { content_index: 0, replay_metadata: None }),
        ScriptStep::Update(ProviderUpdate::Usage {
            usage: Usage {
                input: 1_000_000, output: 500_000, cache_read: 250_000,
                cache_write: 125_000, total_tokens: 99, ..Usage::default()
            },
        }),
        ScriptStep::Update(ProviderUpdate::Done { reason: StopReason::Stop }),
    ])]));
    let mut models = Models::new(Arc::new(|| 73));
    models.register(model.clone(), fake)?;
    let context = Context { system_prompt: None, messages: vec![], tools: vec![] };
    let options = StreamOptions {
        auth: Some(RequestAuth::ConfiguredWithoutSecret { source: None }),
        ..Default::default()
    };
    let mut stream = models.stream(model, context, options);
    while let Some(event) = stream.next().await {
        match event {
            ModelEvent::Start { partial } => assert_eq!(partial.usage, Usage::default()),
            ModelEvent::Done { message, .. } => {
                assert_eq!(message.usage.total_tokens, 1_875_000);
                assert!(message.usage.reported && message.usage.cost.priced);
                assert!((message.usage.cost.total - 6.75).abs() <= 1e-12);
            },
            ModelEvent::Error { error, .. } => return Err(error.failure.unwrap()),
            _ => {},
        }
    }
    let absent = Model {
        identity: ModelIdentity { provider: "local".into(), model: "unpriced".into(), operation: "chat".into() },
        protocol: "local/chat".into(), rates: None, headers: Default::default(),
        capabilities: RequestCapabilities::default(), input: vec!["text".into()],
    };
    let explicit_zero = Model { rates: Some(TokenRates::default()), ..absent.clone() };
    assert!(absent.rates.is_none());
    assert_eq!(explicit_zero.rates, Some(TokenRates::default()));
    Ok(())
}
```

The four category estimates here are USD 2, 4, 0.25 and 0.5, summing to USD 6.75
without intermediate rounding. Each estimate uses `(rate / 1_000_000) * tokens`.
Absent rates produce zero arithmetic with `priced = false`; explicitly supplied
zero rates produce zero arithmetic with `priced = true` after a report. These are
estimates, not provider bills, balances or proof of free access. Floating-point
representation and incomplete provider reports limit their precision/completeness.
No tier, separate reasoning price, tool-result accounting or spending aggregation
is implied; separate operations retain separate accounting.

Checked token addition, finite nonnegative rates and finite category/total costs
are required when processing a report. Invalid arithmetic produces
`Failure::MalformedStream` and atomically preserves prior valid content/accounting.
Accepted usage, requested/actual identity and estimates survive transport errors,
EOF failures and cancellation. Earlier owned snapshots remain unchanged; later
attempts start unreported again. `complete` drains the same accounting stream.
EOF or cancellation does not turn a partial estimate into a successful bill.

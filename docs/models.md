# Model text access

`maestro-models` provides explicitly registered, credential-free text access.
The registry owns provider/model/operation identities and protocols; adapters
supply updates, while the model stream assembles cumulative text and independent
owned snapshots. Registries are instance-local and the crate has no dependencies.

## Scripted text example

This complete example type-checks without a runtime. Run its async body using
an executor supplied by your application. The integration tests execute the same
behavior with a private standard-library executor; no production executor or
network access is provided here.

```rust
use maestro_models::{
    Context, Failure, Model, ModelIdentity, Models, ScriptedProvider, StopReason,
    ProviderUpdate, Usage, UserMessage,
};
use std::sync::Arc;

async fn scripted_text() -> Result<(), Failure> {
    let mut models = Models::new(Arc::new(|| 1_700_000_000_000));
    let fake = Arc::new(ScriptedProvider::new(vec![vec![
        ProviderUpdate::TextDelta { delta: "hel".into() },
        ProviderUpdate::TextDelta { delta: "lo".into() },
        ProviderUpdate::Done {
            usage: Usage {
                input: 11, output: 7, cache_read: 3, cache_write: 2,
                total_tokens: 23,
            },
        },
    ]]));
    let model = Model {
        identity: ModelIdentity {
            provider: "scripted:example".into(),
            model: "greeting/text".into(),
            operation: "chat".into(),
        },
        protocol: "scripted/text".into(),
    };
    models.register(model.clone(), fake.clone())?;
    let context = Context {
        system_prompt: Some("Reply with a greeting".into()),
        messages: vec![UserMessage {
            content: "hello".into(),
            timestamp: 1_699_999_999_999,
        }],
    };
    let response = models.complete(model.clone(), context.clone()).await;
    if let Some(failure) = response.failure {
        // Display text contains only the fixed failure category.
        eprintln!("{failure}");
        return Err(failure);
    }
    assert_eq!(response.stop_reason, Some(StopReason::Stop));
    assert_eq!(response.content[0].text, "hello");
    assert_eq!(response.timestamp, 1_700_000_000_000);
    assert_eq!(fake.calls(), vec![(model, context)]);
    assert_eq!(fake.pending(), 0);
    Ok(())
}
```

## Streaming and completion

Use `models.stream(model, context)` and repeatedly await `stream.next()` for
incremental access. Successful text emits exactly `Start`, `TextStart`, one
`TextDelta` per supplied chunk, `TextEnd`, then `Done`. The block index is zero;
`TextEnd` contains the full text. Successful responses without any text deltas
emit only `Start` and `Done`, without inventing a text block. The first event
requires only the first update, not the whole response. No background task or
channel is used.

Each event owns its content and usage. Retaining or modifying a caller-owned
snapshot cannot affect another event. Initial usage counters are zero because
usage is unreported, not because consumption was measured as zero. Final reported
usage is applied before `TextEnd` and `Done`; no token estimation or pricing is
performed. Partial snapshots have no stop reason. Terminal records carry `Stop`
or `Error`. The requested provider/protocol/model and the clock sampled once at
invocation remain unchanged throughout the response.

`complete` drains exactly one call to the same streaming implementation and
returns its terminal assistant record. Neither method requires credentials or
performs network access itself. A different implementation of `Provider` and
`ProviderStream` can be registered without editing the caller.

## Failures and adapter observations

Only `chat` is implemented. Unknown providers and models are distinct failures;
other operations, unsupported adapter capabilities and mismatched protocols
fail without dispatching an adapter. Exact duplicate identities are rejected
without changing the original registration. Identifiers are opaque strings, not
parsed provider/model shortcuts.

Setup and source failures use the normal `Error` event, never a successful
`Done`. A failure before any successful update can produce only `Error`; a later
failure preserves accumulated text. Premature source EOF is `IncompleteStream`.
After termination the source is dropped and `next()` returns `None` forever.
`Failure` implements `Display` and `std::error::Error` using useful fixed text;
it never includes arbitrary identifiers, context, credentials or raw adapter
errors. Typed categories allow recovery without parsing that display text.

`ScriptedProvider::calls()` returns an independent owned request-history snapshot.
`pending()` reports queued responses. Each invocation consumes at most one
sequence; exhaustion records the call and fails with `ScriptExhausted` instead
of replaying the previous response. Explicit chunks and usage are deterministic;
the scripted adapter performs no credential lookup or external I/O.

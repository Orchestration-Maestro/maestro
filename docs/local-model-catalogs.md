# Local model catalogs

A catalog is locally materialized model metadata bound to explicitly supplied
adapters. The application loads configuration before supplying a trusted,
nonblocking getter. `register_catalog` calls that getter once; a local reload is
another supplied registration, not discovery or remote refresh. Known lookup is
offline and returns owned copies. Identity includes provider, model and operation;
`Some("chat")` excludes other operations. Registry order is not selection policy.

## Registration, overrides and removal

```rust
use maestro_models::*;
use std::sync::Arc;

let base = Model::custom(
    ModelIdentity {
        provider: "local:example".into(),
        model: "text/example".into(),
        operation: "chat".into(),
    },
    "local:chat".into(),
    "local:endpoint".into(),
);
let adapter = Arc::new(ScriptedProvider::new(vec![]));
let mut models = Models::new(Arc::new(|| 73));
models.register_catalog("local:example", || Ok(vec![base.clone()]), adapter.clone())?;
assert_eq!(models.known(Some("chat")), vec![base.clone()]);

// Endpoint-only overrides retain the catalog and its adapters.
models.set_override("local:example", CatalogOverride {
    endpoint: Some("local:alternate".into()),
    models: vec![],
})?;
assert_eq!(models.find(&base.identity).unwrap().endpoint, "local:alternate");

// Clone a model for a partial edit; replacement is otherwise complete.
// The full replacement wins over the provider endpoint, for this operation only.
let mut changed = base.clone();
changed.name = "Alternate display".into();
changed.rates.input = 2.0;
changed.rates_supplied = true;
models.set_override("local:example", CatalogOverride {
    endpoint: Some("local:alternate".into()),
    models: vec![changed.clone()],
})?;
assert_eq!(models.find(&base.identity), Some(changed.clone()));

// Invalid replacements publish nothing and preserve the effective catalog.
let mut invalid = base.clone();
invalid.endpoint.clear();
assert_eq!(models.register_catalog("local:example", || Ok(vec![invalid]), adapter),
    Err(Failure::InvalidCatalog));
assert_eq!(models.find(&base.identity), Some(changed));
assert!(models.remove_override("local:example"));
assert_eq!(models.find(&base.identity), Some(base));
assert!(!models.remove_override("local:example"));
assert!(models.remove_provider("local:example"));
assert!(models.known(None).is_empty());
# Ok::<(), Failure>(())
```

Provider endpoint overrides apply first; full operation-qualified model
replacements apply second, including their endpoint. Unmatched replacements wait
for a matching base registration; they never advertise or implement a model.
Replacing a base catalog retains overrides but cannot resurrect removed models.
Removing an override restores the newest base, not the base at override creation.
An empty valid catalog removes base entries while retaining overrides; removing
a provider removes both. Single-entry `register` still rejects duplicate identities
without replacing their original adapter.

## Known versus available

```rust
use maestro_models::*;

fn inspect(models: &Models, resolver: &dyn AuthResolver) {
    let known = models.known(Some("chat"));
    // Application-owned selected-account restrictions, using metadata only.
    let available = models.available(Some("chat"), resolver,
        &|model| model.identity.model != "disallowed-for-this-account");
    for candidate in available {
        assert!(candidate.auth_status.configured);
        assert!(known.contains(&candidate.model));
    }
    // With no selected-account restrictions:
    let unrestricted = models.available(None, resolver, &|_| true);
    assert!(unrestricted.iter().all(|entry| entry.auth_status.configured));
}
```

Available lookup samples `AuthResolver::status` once per represented provider and
invokes the supplied account predicate only for configured entries. It never
resolves credentials, exchanges tokens or dispatches requests. Explicit
secret-free configuration can be available. Configured status and source are
metadata, not live validation: a later request can still fail authentication.
The predicate must be nonblocking and metadata-only; credential stores and
account selection belong to the application.

## Caller-independent adapter replacement and captured requests

```rust
use maestro_models::*;
use std::sync::Arc;

async fn replace_adapter(
    models: &mut Models,
    model: Model,
    context: Context,
    replacement: Arc<dyn Provider>,
) -> Result<(), Failure> {
    let options = StreamOptions {
        auth: Some(RequestAuth::ConfiguredWithoutSecret { source: None }),
        ..Default::default()
    };
    let mut in_flight = models.stream(model.clone(), context.clone(), options.clone());
    // The same caller interface accepts an independently implemented adapter.
    models.register_catalog(&model.identity.provider, || Ok(vec![model.clone()]), replacement)?;
    // The captured request keeps its old adapter, metadata and header inputs.
    while in_flight.next().await.is_some() {}
    // Only subsequent requests use the replacement; complete drains one stream.
    let next = models.complete(model, context, options).await;
    assert!(next.stop_reason.is_some());
    Ok(())
}
```

`stream` synchronously captures effective registered metadata and the adapter,
including before lazy selected-provider authentication starts. Provider/model/request
header overlays are captured too. Changes before first polling, during credential
resolution or during body reads cannot rewrite that request. No registry lock is
held across external callbacks or polling. Caller-edited endpoint, headers, rates
or capabilities are not authoritative; dispatch uses current effective metadata.
The supplied protocol must still match: obtain a fresh model after a protocol
change. Advertising an operation does not implement it; adapter support is
checked independently.

## Metadata and validation

Custom chat metadata defaults to ID-as-name, text input, false reasoning, no
explicit thinking mappings, context 128,000 and output 16,384. These are fallback
declarations, not measured limits. Four default arithmetic zero rates with
`rates_supplied = false` mean unspecified pricing, not free access. Explicit zero
rates use `rates_supplied = true`. Non-chat construction invents no chat limits
or input capabilities. Thinking mapping keys are open strings; absent keys differ
from explicit `None` (disabled). No thinking resolution or cost arithmetic occurs.

Registration and overrides reject empty required identifiers, protocol, endpoint,
name or input identifiers; negative/nonfinite rates; zero declared chat limits;
and incorrect chat metadata placement. `None` limits are unspecified. Identifiers,
protocols and endpoints are opaque nonempty strings, including slashes and colons;
there is no URL parsing or credential requirement. Whole batches validate before
publication. Duplicate complete keys return `DuplicateModel`; provider mismatch
or other invalid metadata returns `InvalidCatalog`. Getter errors normalize to
fixed `CatalogFailed` diagnostics, preserving old usable entries and adapters.
Endpoint and header values are redacted in model Debug, never interpolated into
these diagnostics. Literal headers retain request-time validation and precedence.

There is no filesystem reader, remote catalog runtime, account selection,
session reconciliation or credential lifecycle inside this registry.

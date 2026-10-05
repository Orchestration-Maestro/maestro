# Request authentication

`maestro-models` accepts authentication for one selected provider. Credential
lifecycle belongs to the credential owner outside models: persistence, precedence
among stored accounts, secret commands, expiry checking, login and refresh
serialization are not model responsibilities. No credential store or asynchronous
runtime is needed for explicit requests.

## Explicit authentication

Explicit authentication wins over the injected resolver, including when an empty
secret is invalid. Neither resolver method is called to repair explicit input.

```rust
use maestro_models::*;

async fn explicit(models: &Models, model: Model, context: Context) -> AssistantMessage {
    let options = StreamOptions {
        auth: Some(RequestAuth::Secret {
            secret: SecretString::new("synthetic-request-token".into()),
            source: Some("runtime".into()),
        }),
        ..Default::default()
    };
    models.complete(model, context, options).await
}
```

`SecretString::expose()` is deliberate access for an authorized adapter or
credential owner. Debug emits a fixed marker, without contents or length.
It does not implement Display or serialization. Redaction is not memory
zeroization: cloned secrets remain owned sensitive values. Source labels and
credential names must be non-secret. Admitted adapters and resolvers are trusted
to avoid copying credentials into response content or diagnostics; models do not
scrub generated content heuristically.

## Injected resolution

A resolver supplies exactly the selected provider's request authentication.
Resolution starts lazily on the first asynchronous stream read, once per request;
models never cache credentials. Adapters receive `ProviderOptions`, not the
resolver or caller's unresolved options.

```rust
use maestro_models::*;
use std::{future::Future, pin::Pin, sync::Arc};

struct LocalResolver;
impl AuthResolver for LocalResolver {
    fn status(&self, provider: &str) -> AuthStatus {
        AuthStatus {
            configured: provider == "local",
            source: Some("application-config".into()),
        }
    }
    fn resolve(
        &self,
        provider: String,
        cancellation: Cancellation,
    ) -> Pin<Box<dyn Future<Output = Result<RequestAuth, Failure>> + Send + '_>> {
        Box::pin(async move {
            if cancellation.is_cancelled() { return Err(Failure::Cancelled); }
            if provider != "local" { return Err(Failure::MissingAuthentication); }
            Ok(RequestAuth::ConfiguredWithoutSecret {
                source: Some("application-config".into()),
            })
        })
    }
}

async fn resolved(models: &Models, model: Model, context: Context) -> AssistantMessage {
    let resolver = Arc::new(LocalResolver);
    let metadata = models.auth_status(&model.identity.provider, resolver.as_ref());
    // Supplied metadata does not promise request success.
    let _ = metadata;
    models.complete(model, context, StreamOptions {
        auth_resolver: Some(resolver),
        ..Default::default()
    }).await
}
```

`auth_status` rejects unknown providers and otherwise reads only resolver
metadata. `status` must not resolve, refresh, read secret values or run commands.
Configured metadata is not live validation and can coexist with a later failed
request. Resolver future creation and provider metadata/setup callbacks must not
synchronously block.

## Local secret-free access

Secret-free endpoints require explicit configuration, not a magic token or a
missing credential. Default options alone do not authorize access.

```rust
use maestro_models::*;

async fn local(models: &Models, model: Model, context: Context) -> AssistantMessage {
    models.complete(model, context, StreamOptions {
        auth: Some(RequestAuth::ConfiguredWithoutSecret { source: None }),
        ..Default::default()
    }).await
}
```

## Headers and failures

Before auth resolution, models validate literal string headers and overlay
provider description defaults → registered model defaults → request values.
Names match ASCII-case-insensitively; effective adapter keys are lowercase.
Later layers replace earlier values, including empty strings, without mutating
inputs. Header names must be nonempty HTTP field-name tokens. Values cannot
contain control bytes except horizontal tab. Case-folded duplicates within one
input map are rejected. There is no null deletion, environment expansion or
final-header callback. Invocation model headers do not replace registered
metadata; adapters use the effective options map without re-overlaying defaults.

All header values, including unknown names, are redacted in Debug for `Model`,
`StreamOptions`, `ProviderOptions` and `ProviderDescription`. The maps themselves
are deliberate sensitive input/adapter surfaces and must not be logged directly.
Authentication and headers are never copied into context, events or terminal
records by models.

Missing input or empty secrets produce `MissingAuthentication`; resolver failures
produce `AuthenticationFailed`. A resolver's other failure categories normalize
to AuthenticationFailed except MissingAuthentication and Cancelled. Invalid
header setup produces `InvalidRequestHeaders` before resolution. These failures
emit one Error with no Start, no successful Done and no adapter dispatch. Fixed
failure text contains no external cause. There is no fallback to environment
credentials or another provider, and no retry.

## Cancellation and exchange ownership

Cancellation while resolving auth wakes a blocked read, drops its retained local
future and prevents dispatch. It emits one Aborted/Cancelled result with no
Start. Observed readiness ties favor cancellation. Dropping a pending `next()`
future does not restart resolution; dropping the stream releases it. Cancellation
stops local work or waiting, not remote effects already performed. Completion
drains the same single stream and shares these semantics.

`ProviderDescription::ambient_credential_names` is arbitrary inert name data,
not an environment reader. `Provider::token_exchange()` defaults to None; an
adapter may return an optional `TokenExchange` primitive. Only the credential
owner calls it under its own lock. Obtaining the handle, status checks and model
requests never execute exchange. The owner supplies opaque `SecretString` state
and cancellation; the primitive returns request auth, rotated opaque state and
optional absolute Unix-millisecond expiry. Models do not interpret or persist
that state, compare expiry, schedule refreshes, set a validity window or timeout,
or coordinate locks. ScriptedProvider has no exchange. No concrete network
connection or real refresh implementation is provided here.

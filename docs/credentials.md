# Provider credentials

`maestro-credentials` owns provider credential policy. Models receive only
request-scoped `RequestAuth` for the selected provider. Storage and secret access
are replaceable without changing the caller's construction or request code.
There is no provider roster, ambient installation discovery, token exchange,
login UI or application wiring here.

## Storage and explicit locations

Inject `Arc<dyn CredentialStorage>` into `Credentials::new`:

- `MemoryCredentialStorage::new(None)` is isolated and nonpersistent.
- `FileCredentialStorage::new(absolute_path)` binds one explicit file without I/O
  during adapter construction. Credential-owner construction reads synchronously.
- `ReadOnlyCredentialStorage::new(storage)` wraps the same adapter, permitting
  reads and rejecting all replacements, including byte-identical replacements.

File operations lock an independent handle using native exclusive locks over
stable file identity. Full read/parse/change/write happens under ownership, with
10 attempts: the first immediate, at most nine 20-ms waits after contention.
No locked file is renamed or lock state unlinked. New files are created with
0600 and new parent directories with 0700 where supported; existing directories
are not recursively chmodded. Missing stores start empty; only an exclusively
created file may initialize its empty bytes to `{}` under its acquired guard.
An existing empty file is malformed.

Private files are plaintext, not encrypted storage. Locks are advisory protection
against cooperating writers, not hostile writers. Failed writes are reported;
there is no crash-durability, rollback or interrupted-write recovery guarantee.
Memory conformance does not establish persistent durability or platform support.

## Resolution and metadata

Explicit model-request auth bypasses the credential owner. Within the owner:

1. A present already-resolved runtime override wins without storage access.
2. A selected stored API key resolves through the secret adapter. Stored token
   auth is available only with a known future expiry (`now < expires_at`).
3. Only successful reads with no selected record permit registered environment
   names, in their supplied order, followed by the configured fallback resolver.

Provider IDs and environment names are arbitrary supplied registration data.
No names are inferred from a provider ID. Runtime overrides are never persisted;
clearing one restores normal resolution. Stored refreshable state is opaque and
is not interpreted or exchanged. Equal, past and unknown expiry are unavailable
until token refresh is implemented.

Present but unusable stored/runtime input never falls through to another account.
Empty auth, unresolved helpers and unavailable tokens produce model
`MissingAuthentication`; malformed selected records/read failures produce
`AuthenticationFailed`. Fallback failures are normalized to the model interface.
Owner-generated sources are fixed labels: `runtime`, `stored`, `environment`,
`fallback`.

`list`, `status` and model `auth_status` inspect detached metadata without reading
storage again, environment values, helpers or fallback resolution. Runtime/stored
presence reports configured even when a later request fails. Declared environment
names report source `environment`, configured false; otherwise fallback metadata
supplies the boolean with source `fallback`. Metadata is not live validity.

## Current format and preservation

The sole format is a JSON object keyed by arbitrary provider IDs:

```json
{
  "synthetic": { "type": "api_key", "key": "SYNTHETIC_KEY" },
  "token-example": {
    "type": "refreshable",
    "auth": { "secret": "SYNTHETIC_TOKEN" },
    "state": "opaque provider state",
    "expires_at": 123
  }
}
```

Secret-free auth uses exactly `{"without_secret":true}` instead of `secret`;
unknown expiry is null. Exactly one auth form is permitted. Provenance labels
are reconstructed, not persisted. Unknown fields/payloads for unrelated providers
are retained as JSON values when one provider changes. A selected record is
validated before use. Invalid JSON, an existing empty file or non-object root is
malformed, never credential absence, and cannot be overwritten by owner updates.

`set`, `remove` and `reload` are synchronous. They serialize mutation/publication
per owner and publish new metadata only after success. Failed reload keeps the
last valid metadata but does not authorize ambient request fallback. `remove`
changes local stored data only: no remote revocation or runtime/ambient clearing.

## Secret helpers

Bind `NativeSecretResolver` to an explicit absolute working directory. Adapter
construction and metadata inspection never start a helper. Only a leading `!`
requests shell execution; strip that first character, without interpolation or
escape grammar. Otherwise a nonempty environment variable whose name equals the
whole value wins, then the unchanged literal. Empty final values are unavailable.

Helpers close stdin, capture stdout, discard stderr and time out after 10,000 ms.
Trim surrounding stdout whitespace, preserving interior newlines. Empty output,
nonzero exit, spawn/UTF-8 failure and timeout resolve unavailable without exposing
command text or diagnostics. Success and unresolved failure are process-cached by
(explicit working directory, full `!command`) across instances. Same-key waiters
share one execution. Environment/literal values are not cached.
`reset_secret_helper_cache()` clears completed results and prevents older in-flight
work from repopulating the cleared generation.

Read-only storage is not helper containment: resolving an explicitly stored
helper can still execute a program under normal operating-system permissions.

## Cancellation and safe errors

Resolver futures delegate blocking storage/process work to owned workers.
Creation/polling does not wait on files/processes. Cancellation wakes local
waiting, wins readiness ties and prevents model dispatch. A cancelled storage
waiter never admits its callback or releases another operation's guard; an
already admitted write keeps ownership until it settles. Cancellation is not
rollback. Cancelling one helper waiter does not cancel unrelated same-key waiters
or cache cancellation as helper failure. Owned native work retains cleanup/reap
responsibility; stopping local waiting does not undo arbitrary helper effects or
contain descendants.

`CredentialError` has fixed secret-safe categories: `Cancelled`, `ReadOnly`,
`Contended`, `Malformed`, `Storage`, `InvalidPath`, with no external error source.
Sensitive inputs use redacted `SecretString`/`RequestAuth` Debug, not zeroization.
The authorized transaction callback may inspect sensitive bytes; ordinary listing,
metadata, errors, model events and transcripts do not export credentials.

## Synthetic configuration

```rust,ignore
let path = explicit_absolute_root.join("credentials.json");
let storage = Arc::new(FileCredentialStorage::new(path)?);
let read_only = Arc::new(ReadOnlyCredentialStorage::new(storage));
let secrets = Arc::new(NativeSecretResolver::new(explicit_working_directory)?);
// Supply registered environment names, an optional fallback and a clock.
let credentials = Credentials::new(read_only, CredentialOptions {
    environment_names: registered_names,
    fallback: None,
    secrets,
    now: supplied_clock,
})?;
```

The crate-root rustdoc includes a compiling memory-to-credential-owner-to-scripted
model-request example. No `AGENTS.md` change is needed: this capability introduces
no new repository rule, command or approved dependency. Glossary terms live in
`CONTEXT.md`.

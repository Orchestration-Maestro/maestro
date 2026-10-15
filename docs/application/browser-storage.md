# Browser collection storage

The application owns browser-store policy. Both `maestro_app` and
`maestro_web` export `Store`, `SettingsStore`, `ProviderKeysStore`,
`CustomProvidersStore`, `SessionsStore`, `SessionData`, `SessionMetadata`,
`AppStorage`, `get_app_storage`, `set_app_storage`, their configuration records
and the storage traits.
`maestro_web` also exports the backend signature types `IndexDirection`,
`TransactionMode`, `StorageOperation` and `QuotaInfo`.
Backend implementations are supplied by the caller, not constructed by stores.
The WebAssembly SDK also provides concrete native-value get/set/delete through
[the browser database adapter](../browser/storage.md). Its complete application
trait adaptation follows in [#1062](https://github.com/Orchestration-Maestro/maestro/issues/1062).

The settings, provider-key and custom-provider stores have out-of-line key schemas named `settings`,
`provider-keys` or `custom-providers`. `Store::new` owns a supplied schema;
`get_config` returns an independent snapshot. Database and index configurations
are data for adapters; these stores do not create databases or interpret indices.

Bind an `Rc<dyn StorageBackend>` with `set_backend`. Unbound operations return
`io::ErrorKind::NotConnected`. Rebinding affects subsequent operations; an
already-polled operation retains its selected backend while awaiting it.
`get_backend` returns an owned shared handle for custom collection policies.

Settings use serde's typed reads and writes over plain JSON values. Keys and
strings are not normalized; JSON object/array order and finite numeric values,
including signed zero, survive the shared conversion. Missing or stored-null
reads return `None`. Conversion failures return `InvalidData`; adapter errors
propagate unchanged. Provider-key presence is independent of empty key content.

Custom providers retain their literal protocol/family and complete model
records, including thinking-map insertion order. `Presence` distinguishes
omitted, null, empty and populated optional keys/models. Listing obtains keys once, reads each sequentially in adapter
order, skips missing records and stops at the first failure. Each read selects
the then-current backend, so a rebind during listing affects later reads.

## Sessions and application storage

`SessionsStore` uses `sessions` and `sessions-metadata`, each keyed by `id`
with a `lastModified` index. Save and delete request one readwrite transaction
covering both collections, writing/deleting history before metadata. The
adapter owns settlement and rollback. Metadata listing requests the descending
index and retains its returned sequence; latest-session selection stable-sorts
that listing using the host's default-locale comparison of modification strings.

Title updates are intentionally nontransactional: read/write metadata first,
then read/write history, skipping each absent or falsy raw record independently.
Only `title` changes, including on open records; an earlier metadata write remains
visible if a later operation fails. Every read and write selects the currently
bound backend separately.

`save_session` accepts `Agent::state()` directly. It prepares the selected model
and history handles before storage admission, releasing the state read guard
before awaiting. Replacing the agent's history collection later does not replace
the captured collection. Serialization observes its retained owners at the
transaction write. Loaded records have new owners after decoding.

Without supplied metadata, the alias generates a summary with zero usage, empty
preview, all-role message count and separate UTC millisecond timestamps. Supplied
metadata is preserved. Full data uses a nonempty explicit title or the metadata
title, inherits its creation timestamp and gets a fresh modification timestamp.
`load_session` and `delete_session` alias the corresponding full-history operations.
The default browser `SessionData` preserves attachment and artifact history;
custom payloads remain generic and persist when they implement serde.

`AppStorage::new` retains its five supplied handles without binding stores.
Quota and persistence use its own backend. Initialize the realm-local slot with
`set_app_storage` before `get_app_storage`; replacing it keeps previously returned
handles usable.

Transactions, prefix/index ordering, quota and persistence are
[adapter contracts](../../crates/maestro-app/src/browser/storage/types.rs),
not platform implementations here. No browser store redirects into native history.

# Browser collection storage

The application owns browser-store policy. Both `maestro_app` and
`maestro_web` export `Store`, `SettingsStore`, `ProviderKeysStore`,
`CustomProvidersStore`, their configuration records and the storage traits.
Backend implementations are supplied by the caller, not constructed by stores.

Each default store has an out-of-line key schema named `settings`,
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
records. `Presence` distinguishes omitted, null, empty and populated optional
keys/models. Listing obtains keys once, reads each sequentially in adapter
order, skips missing records and stops at the first failure. Each read selects
the then-current backend, so a rebind during listing affects later reads.

Transactions, prefix/index ordering, quota and persistence are adapter contracts,
not platform implementations here. Session collections and application-wide
storage access are separate work. No browser store redirects into native history.

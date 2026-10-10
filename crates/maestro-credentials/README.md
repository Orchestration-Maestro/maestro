# Maestro credentials

Resolve configured credential values and ordered headers, keep stored
credentials behind a replaceable file or memory backend, select request keys,
log in and refresh saved OAuth tokens, and format login guidance.

```rust
use maestro_credentials::{
    ApiKeyCredential, AuthCredential, AuthSource, AuthStorage, AuthStorageData,
};

let storage = AuthStorage::in_memory(AuthStorageData::new());
storage.set("openai", AuthCredential::ApiKey(ApiKeyCredential { key: "OPENAI_KEY".into() }));
assert_eq!(storage.list(), ["openai"]);
assert_eq!(storage.get_auth_status("openai").source, Some(AuthSource::Stored));
```

```rust
use maestro_credentials::{ConfigValueOperations, resolve_config_value};

struct Supplied;
impl ConfigValueOperations for Supplied {
    fn environment(&self, name: &str) -> Option<String> {
        (name == "SERVICE_KEY").then(|| "supplied-key".to_owned())
    }
    fn execute(&self, _command: &str) -> Option<Vec<u8>> { None }
}
assert_eq!(resolve_config_value("SERVICE_KEY", &Supplied).as_deref(), Some("supplied-key"));
```

See [credentials](../../docs/credentials.md) for storage, cache, header and
native helper behavior. Authored documentation paths use `maestro-path`.

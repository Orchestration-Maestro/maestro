# Maestro credentials

Resolve configured credential values and ordered headers, keep stored
credentials behind a replaceable file or memory backend, select request keys,
log in and refresh saved OAuth tokens, and format login guidance.

```rust
use maestro_credentials::{
    ApiKeyCredential, AuthCredential, AuthSource, AuthStorage, AuthStorageData,
};

let storage = AuthStorage::in_memory(AuthStorageData::new());
storage.set("openai", AuthCredential::ApiKey(ApiKeyCredential { key: "OPENAI_KEY".into(), env: None }));
assert_eq!(storage.list(), ["openai"]);
assert_eq!(storage.get_auth_status("openai").unwrap().source, Some(AuthSource::Stored));
```

```rust
use maestro_credentials::{ConfigValueOperations, resolve_config_value};

struct Helper;
impl ConfigValueOperations for Helper {
    fn execute(&self, command: &str) -> Option<Vec<u8>> {
        (command == "print-key").then(|| b"supplied-key\n".to_vec())
    }
}
assert_eq!(resolve_config_value("!print-key", &Helper).as_deref(), Some("supplied-key"));
```

See [credentials](../../docs/credentials.md) for storage, cache, header and
native helper behavior. Authored documentation paths use `maestro-path`.

# Maestro credentials

Resolve configured credential values and ordered headers; format login guidance.
Credential storage and refresh are not provided by this crate yet.

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

See [configured credentials](../../docs/credentials.md) for cache, header and
native helper behavior. Authored documentation paths use `maestro-path`.

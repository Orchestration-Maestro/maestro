# maestro-credentials

Own private provider credentials through replaceable storage and secret adapters.
The only internal dependency points downward to `maestro-models`, which receives
request-scoped authentication rather than stored credential lifecycle.

See [credential behavior and examples](../../docs/credentials.md).
The crate-root example constructs synthetic memory storage and a scripted request.

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped cargo test -p maestro-credentials
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- just check
```

File locks protect cooperating writers on stable file identity. Files are private
plaintext, not an encrypted vault. Memory tests do not establish durability.

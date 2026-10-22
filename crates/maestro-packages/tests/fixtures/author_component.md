# Author component fixture

This is the compiled [author component example](../../../maestro-extensions-wasm/examples/author_component.rs),
using its existing [extension body](../../../maestro-extensions-wasm/src/tests/author.rs).
Discovery tests copy this one artifact; they do not compile or activate it.

- Source revision: `b6b1f67edb5e2042a3103e270544bc2ff22bc860`.
- Toolchain: Rust `1.98.0` (`88d9e12ae`), target `wasm32-wasip2`.
- Size: 2,318,584 bytes.
- SHA-256: `953874aaaf935de05b63952780774ce97afc45ffd6cd0fbb3752a211b80812c1`.
- Component header: `0061736d0d000100`.

Build from the repository root, then copy the resulting example artifact here:

```sh
CARGO_BUILD_JOBS=3 CARGO_PROFILE_RELEASE_STRIP=symbols \
  ~/.local/bin/capped cargo build -p maestro-extensions-wasm \
  --example author_component --target wasm32-wasip2 --release --locked
```

The source hashes at that revision were:

| File | SHA-256 |
|---|---|
| `examples/author_component.rs` | `eccdef7fa301fe9b733f01003b74598a98939372d41172dd02e1e7e16d475dba` |
| `src/tests/author.rs` | `cdffe93876fc7dc8d26e7e0fa966a573bb39ca0d990577ed6bc35cfdfbf97e4a` |
| `Cargo.lock` | `16e2a655b48a962b78ebf70c4174b105e0a568e573323c0becd4e55d016c2a8e` |
| `rust-toolchain.toml` | `324740c82d90982a6a92d47776adc20e42defbbc571ea44c50c82c4ffa185641` |

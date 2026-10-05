# Maestro

Maestro is an agent engine in Rust: the foundation the other Maestro parts
build on. It is being built piece by piece, with specs in `docs/specs/`.

## Build and test

With Rust installed, run these from the repository root. Clippy builds and
checks every target; the tests also build the binary.

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Shared CI runs format and lint on Linux, with tests on Linux, macOS and Windows.

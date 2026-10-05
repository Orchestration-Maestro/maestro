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

## Workspace crate rules

`maestro-test-conventions` runs automatically with the workspace tests. Add
new members to `workspace-crates.json` with a `core` or `dedicated` layer.
The composition root (`maestro`) and the conventions checker are dedicated;
engine libraries such as `maestro-models` are core.

The checker rejects unlisted members, dependency cycles, core-to-dedicated
edges and all internal dependencies of `maestro-models`. Normal, build, dev,
optional and target-specific dependencies all count. External dependencies
are not workspace edges. A dependency with a workspace member's package name
must use a path to that member, not a registry/git source or a different path;
this prevents Cargo patches or overrides from hiding internal edges.
The full per-crate dependency allowlist is deferred.

Names are `maestro` or `maestro-` followed by one or two lowercase ASCII
alphanumeric segments, each starting with a letter. The checked-in crate
list records the reviewed names; the checker does not invent a noun or role
vocabulary.

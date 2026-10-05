# Maestro

Maestro is an agent engine in Rust: the foundation the other Maestro parts
build on. It is being built piece by piece, with specs in `docs/specs/`.

## Build and test

Install `mise` and `rustup`, then run from the repository root:

```sh
mise install
just setup
```

Enable mise in your shell (or prefix commands with `mise exec --`) so the
pinned tools are on `PATH`. Rust comes from `rust-toolchain.toml` through
rustup, not mise. `mise.lock` records the tool downloads and checksums.

- `just setup`: install the pinned tools and git hooks.
- `just check`: formatting, Clippy with warnings denied, strict public
  documentation, and workspace conventions tests.
- `just test`: the full workspace test suite.
- `just ci`: checks and tests, matching shared Linux CI.

Before each commit, prek checks file hygiene and runs `just check`. Commit
messages require a conventional header and lines of at most 80 columns.

## Documentation rules

Every crate root has `//!` documentation and every public item has `///`
documentation. The documentation build denies warnings and missing docs.
Comments describe code behavior, never planning material: no numbered
slices, specs, tasks or tickets; issue or pull-request references; planning
vocabulary or identifiers. The conventions checker scans Rust comments in
each workspace member and reports the file and line. Strings and attributes
are not comments. There are no exemptions; reword false positives.

Technical wording such as "the JSON-RPC specification", "an async task",
"step 1 of the parse", "UTF-16" and "S3-compatible" remains valid.

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

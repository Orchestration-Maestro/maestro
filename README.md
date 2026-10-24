<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/brand/dark/mark.svg">
  <source media="(prefers-color-scheme: light)" srcset="assets/brand/light/mark.svg">
  <img src="assets/brand/light/mark.svg" alt="Maestro mark" width="96" height="96">
</picture>

# Maestro

Conduct every model.

Maestro is an agent engine in Rust: the foundation the other Maestro parts
build on. The implementation is being rebuilt crate by crate, with specs in
`docs/specs/`.

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
  documentation, and the workspace conventions on this checkout.
- `just test`: the full workspace test suite.
- `just ci`: checks and tests, matching shared Linux CI.

Before each commit, prek formats the code with `cargo fmt`, re-stages the
staged files and runs `just check`, then the full conventions suite when a
staged path is under `crates/maestro-test-conventions/`. It also rejects
merge-conflict markers, invalid TOML or YAML, and a secret in the staged change.
Commit messages require a conventional header and lines of at most 80 columns.

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
new members to `workspace-crates.json` with a `core` or `dedicated` class.
Only the conventions checker is currently implemented. Planned crate classes
and dependency boundaries remain defined by the foundation specification.

The checker rejects unlisted members, dependency cycles, core-to-dedicated
normal/build edges and all internal domain dependencies of leaf crates.
Normal, build, dev,
optional and target-specific dependencies all count, subject to the declared
foundation utility, native feature-unifier and internal development exceptions
in the [foundation contract](docs/specs/maestro-port.md#crates-and-delivery-order).
External dependencies are not workspace edges. A dependency with a workspace member's
package name must use a path to that member, not a registry/git source or a
different path; this prevents Cargo patches or overrides from hiding internal
edges.
The per-crate dependency allowlist applies whenever a crate exists.

Names are `maestro` or `maestro-` followed by one or two lowercase ASCII
alphanumeric segments, each starting with a letter. The checked-in crate
list records the reviewed names; the checker does not invent a noun or role
vocabulary.

## Documentation

- [Identity guide and brand pack](docs/identity.md)
- [Foundation specification](docs/specs/maestro-port.md)
- [Architecture checks](docs/architecture.md)
- [Contributor workflow](docs/agents/issue-tracker.md)

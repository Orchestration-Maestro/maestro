<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/brand/dark/mark.svg">
  <source media="(prefers-color-scheme: light)" srcset="assets/brand/light/mark.svg">
  <img src="assets/brand/light/mark.svg" alt="Maestro mark" width="96" height="96">
</picture>

# Maestro

Conduct every model.

Maestro is a Rust agent engine. The native executable connects model
conversations, local file and shell tools, saved sessions and terminal,
print/JSON and RPC interfaces. See
[command-line startup](docs/cli.md), [tools](docs/tools.md) and
[sessions](docs/sessions/session_manager.md) for the delivered behavior.

**Start here: [Your first Maestro session](docs/first-session.md).** Build from
source, connect an account and get a first result in a disposable project.
The [foundation specification](docs/specs/maestro-port.md) describes the wider
architecture; it is not a list of everything available today.

## Before sharing a session

Session files can contain private code, credentials pasted into prompts and shell
output. Review and redact a copy before publishing, and get permission to share
other people's material. A secret gist is readable by anyone with its URL.
[Session publication guidance](docs/first-session.md#next-one-shot-output-or-integration)
explains the export-bundle requirement and the difference between excluding
output from model context and keeping it out of stored history.

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

[How to document in Maestro](docs/how-to-document-in-maestro.md) explains how to
write guide pages, examples, specifications, glossary entries, decision records
and rustdoc. [AGENTS.md](AGENTS.md#documentation) holds the rules the build
enforces.

## Workspace crate rules

`maestro-test-conventions` runs automatically with the workspace tests. Add
new members to `workspace-crates.json` with a `core` or `dedicated` class.
Delivered crate owners and their dependency boundaries are recorded in the
[workspace inventory](workspace-crates.json) and the foundation specification.

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

- [First-session user guide](docs/first-session.md)
- [Consume JSON session events](docs/json-events.md)
- [Library architecture and checks](docs/architecture.md)
- [Approved libraries](docs/libraries/)
- [Identity guide and brand pack](docs/identity.md)
- [How to document in Maestro](docs/how-to-document-in-maestro.md)
- [Foundation specification](docs/specs/maestro-port.md)
- [Contributor workflow](docs/agents/issue-tracker.md)

## License

Maestro is [MIT licensed](LICENSE). Dependencies retain their own licenses;
[approved library notes](docs/libraries/) record the selected libraries.

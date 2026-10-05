# Working on Maestro

## Flow

Land a spec on `main` under `docs/specs/` first. Then use `/to-tickets`,
`/implement` (with `/tdd`, then `/code-review`), `/pr`, and `/retro`.
Issues link to the spec rather than copying it.

## Checks

Install mise and rustup, enable mise on `PATH`, then run:

```sh
mise install
just setup
just ci
```

`just setup` installs pinned tools and prek hooks. `just check` runs formatting,
Clippy with warnings denied, strict public rustdoc and workspace conventions.
`just test` runs all tests; `just ci` runs both, matching shared Linux CI.
Rust is pinned in `rust-toolchain.toml` and installed by rustup, never mise.
Use `mise exec -- just <recipe>` if mise is not active in your shell.

Commit hooks run basic file hygiene and `just check`. Commit messages need
a conventional header and every line must be at most 80 columns.

## Documentation

Every crate root needs `//!` docs and every public item needs `///` docs.
`just check` denies rustdoc warnings and missing documentation.

All Rust comments describe the code, not planning material. Never include
numbered slices, specs, tasks or tickets, issue/pull-request references,
planning vocabulary or planning identifiers. Workspace conventions scan
comments in all member Rust files, reporting file and line; strings and
attributes are not comments. No allowlist or exceptions: reword a false
positive. Ordinary technical terms such as "the JSON-RPC specification",
"an async task", "step 1 of the parse", "UTF-16" and "S3-compatible" pass.

## Crates

- One job per crate, describable in one sentence without "and".
- Dependencies point one way; core crates never depend on dedicated
  (non-core) crates.
- Names follow `maestro-<noun>[-<role>]`.
- The composition-root crate, `maestro`, owns the binary and only wires.

## Libraries and formats

Only these libraries are owner-approved: tokio, reqwest, serde, serde_json,
toml, clap, rmcp, tracing, and globset. Use Git through the `git` command.
Ask the owner before adding any other crate; never add one silently.

Keep one current format for everything. No compatibility code.

## Public text

Describe Maestro in its own words. Name no other agent tool.

## Agent skills

### Issue tracker

Issues are GitHub Issues in `Orchestration-Maestro/maestro`; specs land on
`main` first. See `docs/agents/issue-tracker.md`.

### Triage labels

Matt Pocock's five default triage labels. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context, with the glossary in `CONTEXT.md` and ADRs in `docs/adr/`.
See `docs/agents/domain.md`.

# Workspace architecture checks

The [foundation specification](specs/maestro-port.md#crates-and-delivery-order)
owns the crate graph. `maestro-test-conventions` checks that graph through
`check_workspace(root)`; it does not execute product capabilities.

## Inventory and dependencies

The extension domain may depend directly on resources for shared source information. Resources remain a leaf; this permission does not allow the reverse dependency or internal dev-dependencies.

`workspace-crates.json` lists actual workspace members with string-valued classes.
The binary composition root, conventions checker, terminal scenario harness and
repository tooling are `dedicated`; all product libraries and adapters are `core`.
Repository tooling (`maestro-tooling`) is development-only, never shipped and has
no internal dependencies. Classes are not numeric delivery layers. Known absent
entries do not require placeholder crates; unknown actual members are rejected.

Eight leaves have no internal dependencies. Other crates may use a subset of
their specification row, except these complete direct sets:

- CLI and chat: application, toolkit, terminal adapter and theme.
- RPC and web: application and theme.
- Terminal adapter and terminal scenario harness: toolkit only.

Completeness is the union of declared normal/build dependencies across features
and targets plus host-resolved dependencies. Transitive and dev dependencies do
not satisfy a direct requirement. The internal dev-target allowlist is empty;
the scenario harness is not a reusable test-support dependency. Both production
(normal/build) and test (all kinds) graphs must be acyclic before individual edge
policy is checked.

The checker first runs offline `cargo metadata --format-version 1 --no-deps`.
It then detects the compiler host with `rustc -vV` and requests offline,
host-filtered full metadata with default features, not all features. Package IDs
and canonical member paths establish identity, not dependency aliases. Optional,
build, dev and inactive foreign-target declarations remain subject to policy.
Malformed metadata and command failures return diagnostics, not acceptance.

## Bounded ownership checks

Direct `wasmtime`/`wasmtime-*` library declarations belong only to the runtime
adapter; `wasmtime-wasi-http` is excluded everywhere. The terminal toolkit cannot
declare `ratatui`, `syntect` or `two-face`, including aliased, optional, build,
dev and foreign-target forms. A root may transitively use its runtime adapter.
Review also checks equivalent new framework/highlighting libraries; the finite
library-name list is not a general library classifier.

Rust member files, including files outside `src/`, are traversed while skipping
`target/` and `.git/`. Each file is read once; a leading byte-order mark is removed
and physical CRLF is converted to LF before native Rust parsing. Parsing, comment
lexing, production line counts and WIT literals share that canonical text.
Declarations are recognised from native tokens, so macro templates and every item
shape are covered without expansion; attribute payloads are excluded.
Raw identifiers (`r#name`) normalize to their ordinary names. Tool definition/render-context/result-option types
belong to tools; application selector types and selector modules belong to chat.
Qualified uses and re-exports are not declarations. Duplicate tool declarations
are rejected. Planning references in comments retain their separate policy and
line-accurate diagnostics from native compiler-lexer comment spans.
Legal included fragments need not parse as full modules; the compiler owns
Rust validity, while the quality checker still counts their production lines.

WIT checks recognize actual `wit_bindgen::generate!` and
`wasmtime::component::bindgen!` declarations. A `path:` may be a static string or
array of static strings. Cooked strings use Rust escapes (including hexadecimal,
Unicode and line continuations). Physical CRLF within cooked and raw literals
becomes LF through shared source normalization, while escaped carriage returns
remain carriage returns; raw strings otherwise keep their literal contents.
A shorthand string macro argument names a world and
reads the declaring member's default `wit/` directory, not a path named by the
string. Paths resolve relative to the declaring member's Cargo manifest. Every input must
exist inside the canonical guest-authoring member directory, not a copied host
tree. Directory inputs are roots; file inputs use their containing directory.
When host and guest both declare inputs, their canonical root sets must agree;
world names and individual file selections may differ. With one side declared,
ownership still applies but root-set equality is inactive. Inline WIT outside
the guest is rejected. Dynamic paths (`env!`, `concat!`, constants), missing paths
and unresolved declaration shapes require review and do not pass verification.
No owners or inputs means the check is inactive, not that a runtime passed.

These scans are structural, not a Rust semantic analyzer. They do not resolve
arbitrary macros, generated identifiers or computed paths. Manual source review
must establish caller-supplied generic tool context, no tools-to-extension or
application coupling, no application policy in frontends, a wiring-only binary,
and complete WIT code-generation evidence. Current absent owners are tested with
scratch workspaces, not certified as delivered capabilities.

## Quality checks

Clippy's protected `redundant_clone` lint rejects unnecessary copies workspace-wide.
Conventions reject consecutive identical nonempty trimmed documentation lines, ignoring fenced code and resetting at each comment block.
Conventions reject adjacent token-identical assertion statements in the same test block only when their arguments contain no calls, method calls, macros, assignments or compound assignments; opaque arguments and repetition across functions or files remain review judgement.

The root manifest must forbid every protected quality lint; member manifests
are parsed as TOML and must set the boolean `lints.workspace` to `true`.
The compiler rejects protected-lint allowances, including attributes emitted by
macros and conditional attributes. Lint-group allowances fail because the
workspace also forbids `forbidden_lint_groups`, which ignores warnings denial.
Test files are exempt from the 500-line production limit. Documentation tokens
(`///`, `//!`, `/** */`, `/*! */`) are excluded in complete Rust modules and
included expression fragments alike. Files that parse as Rust modules also
exclude items carrying `cfg(test)`, using source line and column spans regardless
of attribute order. A non-blank line counts when any non-whitespace character
remains outside those exclusions: code sharing a documentation or test line still
counts. Ordinary comments and string contents count. Blank lines are excluded.
A trailing test item's comment-only suffix is excluded only when no production
item follows. Rust validity belongs to the compiler.

## Commands and failure evidence

Run `just check`, `just test` and `just ci` with pinned tools. `just check` includes
formatting, Clippy with warnings denied, strict public rustdoc and conventions;
`just ci` is the shared merge check. Workstation Cargo commands, including nested
metadata and hook commands, use the capped launcher with `CARGO_BUILD_JOBS=3`.
As #109 specifies, `just test` remains unwrapped; the separate ported non-LLM test
script removes the specified provider credentials and sets the agent's auth file
aside, restoring it on exit.

For example, a scratch workspace with listed settings and models members cannot
add this settings manifest declaration:

```toml
[dependencies]
maestro-models = { path = "../models" }
```

`check_workspace` returns:

```text
maestro-settings must not depend on workspace crate maestro-models
```

Removing the dependency passes. An otherwise empty CLI fixture instead reports
`maestro-cli: missing required direct dependency maestro-app`; restoring all four
direct dependencies passes. Source failures include the offending file and line.

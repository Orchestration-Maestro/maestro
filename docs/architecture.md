# Workspace architecture checks

The [foundation specification](specs/maestro-port.md#crates-and-delivery-order)
owns the crate graph. `maestro-test-conventions` checks that graph through
`check_workspace(root)`; it does not execute product capabilities.

## Inventory and dependencies

The extension domain may depend directly on resources. The crates that list
`maestro-request` in the specification table may depend on it; its job is to define shared
message and model-request records, and it has no workspace dependencies.
No reverse dependency is permitted.

The [specification](specs/maestro-port.md#crates-and-delivery-order) declares five more
owners; `maestro-text`, `maestro-json` and `maestro-entry` exist, and tool and brand are policy only with no crate
directory until their extraction issues: `maestro-text` and `maestro-json` are leaves, `maestro-entry` and `maestro-tool` depend
only on `maestro-request`, and `maestro-brand` depends on text and JSON. Pattern depends
only on text, so it is no longer a leaf. Tools may also depend directly on text for shared newline
normalization. The guest authoring crate may depend on request,
entry, tool and JSON, never on native session, tool or catalog runtimes. The table is not
repeated here.

The declared `maestro-export` owner may depend directly on `maestro-json` and
`maestro-text` for shared JSON and whitespace semantics. It may not depend on
`maestro-web`; the application supplies the viewer bundle, as described in the
[session export contract](specs/maestro-port.md#r04--session-trees-persistence-and-exports).
The `export_reuses_text_and_json_without_frontend_edge` architecture test checks
these permissions for normal, build, optional and foreign-target declarations.

`workspace-crates.json` lists workspace members and declared-but-absent owners
with string-valued classes.
The binary composition root, conventions checker, terminal scenario harness and
repository tooling are `dedicated`; all product libraries and adapters are `core`.
Repository tooling (`maestro-tooling`) is development-only, never shipped and has
no internal dependencies apart from the optional utility. Classes are not numeric
delivery layers. Known absent entries do not require placeholder crates; unknown
actual members are rejected.

The specification table identifies leaves with no internal dependencies; the
foundation utility `maestro-path` is also a leaf. The
[architecture conventions test](../crates/maestro-test-conventions/tests/architecture.rs)
checks the table against the enforced graph policy. Frontend dependency requirements
follow the [specification's dependency boundary](specs/maestro-port.md#common-semantic-and-boundary-contract).
Other crates may use a subset of their specification row, except the terminal adapter
and terminal scenario harness: both require the toolkit. The adapter also requires
`maestro-text`; an absent required toolkit or text owner fails. The scenario harness
may not import text.

Completeness is the union of declared normal/build dependencies across features
and targets plus host-resolved dependencies. Transitive and dev dependencies do
not satisfy a direct requirement. The sole permitted internal development edge is
`maestro-tui-crossterm` to `maestro-brand` for the terminal examples. The exception
permits neither a normal/build brand dependency nor a theme dependency, and does
not satisfy the adapter's required toolkit/text dependencies. The scenario harness
is not a reusable test-support dependency. Both production
(normal/build) and test (all kinds) graphs must be acyclic before individual edge
policy is checked.

The checker first runs offline `cargo metadata --format-version 1 --no-deps`.
It then detects the compiler host with `rustc -vV` and requests offline,
host-filtered full metadata with default features, not all features. Package IDs
and canonical member paths establish identity, not dependency aliases. Optional,
build, dev and inactive foreign-target declarations remain subject to policy.
Malformed metadata and command failures return diagnostics, not acceptance.

The cancellation leaf has no internal dependencies; only models and the toolkit
may depend directly on it.

The [shared filesystem effect contract](specs/maestro-port.md#crates-and-delivery-order)
declares `maestro-watch` and `maestro-lock` as core leaves below layer 0, including
when their implementations are absent. Neither may depend on a workspace crate,
including `maestro-path`. Only theme/application may consume watch; only
settings/credentials may consume lock. Watch creation/close, injected watch/timer
interfaces and native notifications belong to watch; bounded synchronous
contention acquisition of caller-opened files belongs to lock. Feature owners
retain reload, refresh, retry, polling, file creation, permissions, persistence,
unlocking and asynchronous credential acquisition policy. This changes ownership,
not public capabilities or feature-specific timing, process, subscription and
teardown semantics; it introduces no generic executor or scheduling policy.

The process leaf `maestro-process` is declared core below layer 0 with no workspace
dependencies, including path and cancellation. Only tools, packages, credentials,
the application and the toolkit may consume it, on native targets. It owns child-process lifecycles through its `child_process` module
(captured spawn, raw pipe collection, exit and EOF observation, termination and
reaping, with a caller-selected post-exit grace and TERM-then-KILL escalation); callers keep cancellation, termination, settlement and output policy.
Synchronous capture follows the timeout handling in the
[contract](specs/maestro-port.md#crates-and-delivery-order). The checker verifies
dependency ownership, not target predicates.

## Authored paths and the foundation utility

An authored path, the text a user, setting or file wrote, stays a `String` and
goes through `maestro-path`; it becomes a `std::path::PathBuf` only at a
file-system call. The utility offers `join`, `normalize`, `resolve`, `relative`,
`dirname`, `basename` and `is_absolute` for the `posix` and `win32` flavors, plus
a POSIX `parse`, with the results of the JavaScript runtime's `path` module,
except where its result is a defect; the crate's tests mark each one. The crate
root re-exports the flavor of the compile target. Nothing reads the process
working directory or the environment: `resolve` and `relative` take a `Cwd`
snapshot from the caller, and `try_resolve` resolves what the operands anchor and
fails with `CwdUnavailable` where the runtime would have read the working
directory. `resolve_with` resolves several operand groups in the target flavor
from callbacks the caller supplies: it calls the drive callback once, and the
working-directory callback once, only for the groups the operands and drives
leave open, and returns that callback's error unchanged. `homedir` returns a
supplied home, empty included, and calls its account callback only when none
was supplied. The callbacks decide what is read and when; the crate reads
nothing itself. Windows relative paths require matching resolved
root identities, including anchoring. A matching anchored drive, share or
namespace supplies a comparison base; otherwise, nonempty tails need a complete
shared first component. Empty tails still permit component traversal. Without a
shared base the normalized destination is returned: `D:a` to `D:b` under a UNC
working directory with no drive entries yields `D:b`, not `..\b`.

`maestro-path` has no internal dependencies and is the one named exception to
leaf and exact-set rules. Any crate in the specification table may declare it as
a normal or build dependency (optional and target-specific forms included)
without changing its layer, except the guest authoring crate, the runtime
adapter, shared request owner, cancellation leaf, watch leaf, lock leaf, process leaf,
file-pattern crate, text leaf, JSON leaf, entry and tool record crates, brand crate and the
terminal scenario harness. Shared path helpers take supplied directory observations and
add no global environment, filesystem service, directory cache, scheduler or workspace
dependency. The edge is ignored when checking a crate's complete
direct set or leaf
status. The
[architecture conventions test](../crates/maestro-test-conventions/tests/architecture.rs)
checks permitted table and utility edges. Dev-dependencies on the utility,
dependencies from the utility, and cycles are rejected like any other internal edge.

## Bounded ownership checks

Direct `wasmtime`/`wasmtime-*` library declarations belong only to the runtime
adapter, except that the guest-authoring crate may declare `wasmtime` and
`wasmtime-wasi` as development dependencies for its test-only host probe;
`wasmtime-wasi-http` is excluded everywhere. The terminal toolkit cannot
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
belong to tools, except the guest author definition in
`maestro-extensions-wasm/src/types/tools.rs`; application selector types and selector
modules belong to chat.
Qualified uses and re-exports are not declarations. Duplicate tool declarations
within either owner are rejected. Planning references in comments retain their separate policy and
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

Clippy rejects the ambiguous `serde_json::Map::remove` in every target `just check`
compiles. `shift_remove` keeps the remaining key order; `swap_remove` is only for
maps whose remaining order is never observed. Ordinary maps are unaffected.

Tests wait on completion signals; shared CI bounds the whole job. Conventions
report recognized test-owned timer calls with file and line: Tokio
`timeout`/`timeout_at`, standard `recv_timeout`, `recv_deadline`, `wait_timeout`,
`wait_timeout_while` and `std::thread::park_timeout`. Test ownership comes from
test file layouts, test-support members and explicit `cfg(test)` scopes,
including impl/trait methods and associated-constant initializers.
Direct paths and known imported, renamed, grouped or namespace-alias timer forms
are checked; known timer-module globs are recognized. Direct timer paths take
precedence over aliases; competing known aliases are retained. Aliases apply file-wide;
local shadowing is not modelled. Production deadlines remain allowed. Comments
and literal contents are not call syntax. The scan does not expand macros,
infer receiver types, resolve arbitrary re-exports/function values, evaluate
compound configurations or follow unparsed fragments. These forms and sleep-based
watchdogs remain source-review obligations.

Clippy's protected `redundant_clone` lint rejects unnecessary copies workspace-wide.
Hand-written documentation uses only `///` and `//!`; conventions reject block doc comments with their file and line, except in generated guest bindings at `maestro-extensions-wasm/src/bindings.rs`. Line documentation in that file is still checked for repetition; allowed block comments end the comparison run.
Conventions reject consecutive identical nonempty trimmed physical documentation lines within one contiguous run of same-style line documentation comments, retaining list and quote markers. An attribute, an ordinary comment or code ends the run. All Markdown code blocks (including nested list and block-quote fences) reset comparison and are ignored. Blank documentation lines are retained and do not reset comparison; owner/style changes do. Headings, rules and HTML lines are compared in their physical order like other non-code lines.
Under `crates/*/src`, files carrying `#![cfg(test)]` must be named `tests.rs` or be below a `tests` directory. A module named `tests` must carry `#[cfg(test)]`, every `#[cfg(test)]` module must be named `tests`, and test module declarations cannot carry `#[path]`. These layout conventions support production line counting; module declarations are validated throughout the syntax tree.
Duplicate assertions are a review judgement.

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

The guest-authoring crate alone declares its own lint table: the workspace table
with `pedantic`, `too_many_arguments` and `excessive_nesting` lowered from
`forbid` to `deny`, because generated bindings cannot compile under `forbid`.
Only that crate's generated `bindings.rs` may carry a lint attribute, the single
allowance `clippy::same_length_and_capacity`. Every handwritten
top-level module, integration test and example restores the groups with
`forbid`, nested files inherit it from their parent module, and the crate root
stays wiring-only.

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
forbidden production dependency: maestro-settings -> maestro-models
```

Removing the dependency passes. An otherwise empty CLI fixture instead reports
`maestro-cli: missing required direct dependency maestro-app`; restoring the
application dependency passes. Other listed frontend edges are permitted, not
required, and are declared only when used. Source failures include the offending
file and line.

Integration tests in `maestro-models` and `maestro-tui` share `tests/integration.rs`. Add new
`tests/*.rs` files as unconditional out-of-line modules in that root; support
modules stay in their existing directories. When top-level integration-test
sources exist, the conventions check requires `autotests = false` and one
runnable `integration` target for these crates.

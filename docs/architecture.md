# Workspace architecture checks

The [foundation specification](specs/maestro-port.md#crates-and-delivery-order)
owns the crate graph. `maestro-test-conventions` checks that graph through
`check_workspace(root)`; it does not execute product capabilities.

## Application export layout

`maestro-app` keeps its public imports at the crate root. Private, static
`src/exports/` fragments hold the declarations by area: `browser_storage`,
`browser_tools`, `clipboard`, `configuration`, `credentials`, `extensions`, `json`,
`markdown`, `models`, `packages`, `presentation_data`, `process`, `resources`,
`session`, `settings`, `text` and `tools`. Each area owns its matching `.rs` file;
`exports/mod.rs` aggregates them without changing the original owners or target
gates. Unrelated areas therefore need not edit the same declaration file.

## Inventory and dependencies

Dependency descriptions here state domain edges, subject to the
[native feature-unifier exception](specs/maestro-port.md#native-feature-unifier),
including leaves and exact sets. The unifier itself has no workspace dependencies.

The extension domain may depend directly on resources. The crates that list
`maestro-request` in the specification table may depend on it; its job is to define shared
message and model-request records, including command provenance records
`SlashCommandInfo` and the open `SlashCommandSource` label, and the renderer option
record `MessageRenderOptions`, `CustomMessageInput<T>`, `MessageDelivery`,
`UserMessageDelivery`, `SendMessageOptions` and `SendUserMessageOptions`. Resources and
application presentation data re-export the two command provenance records; session
re-exports the six others; guest authoring re-exports every record except
`CustomMessageInput`, which it defines as its own alias. The declaration scanner rejects
a declaration of any of these records except `CustomMessageInput` outside the request
owner, and duplicates within it.
The request owner depends only on the dependency-free JSON leaf.
No reverse dependency is permitted.

The [specification](specs/maestro-port.md#crates-and-delivery-order) declares five more
owners, all now present: `maestro-text` and `maestro-json` are leaves, `maestro-entry` and `maestro-tool` depend
only on `maestro-request`, and `maestro-brand` depends on text and JSON. Pattern depends
only on text, so it is no longer a leaf. The [Markdown parser](markdown.md)
also depends only on text; the toolkit consumes its owned source records and
application re-exports them as `markdown`. The dedicated conventions checker may
consume the same parser's code-block source ranges for documentation checks.
Tools may also depend directly on text for shared newline
normalization. The guest authoring crate may depend on request,
entry, tool and JSON, never on native session, tool or catalog runtimes. The table is not
repeated here.

Pure retained [session-tree projection](sessions/tree-projection.md) belongs to
`maestro-session`, with frontend access through `maestro-app::export_html`.
Its allowed dependency set is unchanged; it has no frontend or toolkit edge.

The `maestro-export` owner may depend directly on `maestro-json` and
`maestro-text` for shared JSON and whitespace semantics. It may not depend on
`maestro-web`; the application supplies the viewer bundle, as described in the
[session export contract](specs/maestro-port.md#r04--session-trees-persistence-and-exports).
The `export_reuses_text_and_json_without_frontend_edge` architecture test checks
these permissions for normal, build, optional and foreign-target declarations.

`workspace-crates.json` lists workspace members and declared-but-absent owners
with string-valued classes.
The binary composition root, conventions checker, terminal scenario harness,
checked test-process owner, disposable test-directory owner, test tracing owner,
portable fixture owner, repository tooling and the feature unifier
(`maestro-dependencies`) are `dedicated`; all product libraries and adapters are `core`.
Repository tooling (`maestro-tooling`) is development-only, never shipped and has
no internal dependencies apart from the optional utility and the native-only
feature unifier. Classes are not numeric
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
not satisfy a direct requirement. The permitted internal development edges are
`maestro-tui-crossterm` to `maestro-brand` for the terminal examples, the
native-only `maestro-extensions` to `maestro-credentials` edge for extension
tests that build a real model catalog, the native-only `maestro-detection` to
`maestro-text` edge for terminal newline normalization in checked-child tests, the native
`maestro-rpc` to `maestro-theme` and `maestro-tui-crossterm` edges for its standard-stream
child tests, the native-only `maestro-chat` to `maestro-models` edge for interactive-mode
tests that drive a real session through the scripted provider, the native-only
`maestro-cli` to `maestro-listeners` development edge for the controlled print
adapter, and the native development edges from the twenty-three declared callers to the
`maestro-test-process` owner (see the
[specification](specs/maestro-port.md#test-process-owner)). The brand exception
permits neither a normal/build brand dependency nor a theme dependency, and does
not satisfy the adapter's required toolkit/text dependencies. The extension edge
permits no normal/build credentials dependency. The test-process
owner is reachable only through those native development edges, has no outgoing
domain edge and is never admitted by a `maestro-test-*` name pattern.
The directory owner follows the same native-only declaration checks, with the
exact caller list of its
[directory contract](specs/maestro-port.md#test-directory-owner) and no outgoing
domain or utility edge. The tracing owner
follows the same checks with its own five callers; see the
[tracing contract](specs/maestro-port.md#test-tracing-owner). The scenario harness's
recording terminal and manual runtime are the one shared test-effect owner, with
`maestro-cli` as their only declared caller under the same native development
rules. The portable fixture owner has the eight exact development callers in its
[fixture contract](specs/maestro-port.md#test-fixture-owner). Its declarations may
be portable or target-specific; normal/build edges and outgoing domain or utility
edges are forbidden. The policy admits the owner before its member exists.
Both production
(normal/build) and test (all kinds) graphs must be acyclic before individual edge
policy is checked.

The checker first runs offline `cargo metadata --format-version 1 --no-deps`.
It then detects the compiler host with `rustc -vV` and requests offline,
host-filtered full metadata with default features, not all features. Both metadata
children omit `RUSTC_WRAPPER`; the calling process and build commands retain it.
Package IDs and canonical member paths establish identity, not dependency aliases.
Optional, build, dev and inactive foreign-target declarations remain subject to policy.
Malformed metadata and command failures return diagnostics, not acceptance.

The cancellation leaf has no internal dependencies; only models and the toolkit
may depend directly on it.

The [shared filesystem effect contract](specs/maestro-port.md#crates-and-delivery-order)
declares watch below layer 0 without workspace dependencies. Lock depends only
on path for the single native home reader, using the existing SHA-256 dependency
for canonical target identity. Only theme/application may consume watch; only
settings/credentials/workspace/detection/session may consume lock. The lock owner
provides bounded and waiting acquisition plus recoverable in-place byte
publication. See [file locking](locking.md) for the durable copy and replay order.
Feature owners retain serialization, symbolic-link policy and scheduling.

The process leaf `maestro-process` is declared core below layer 0 with no workspace
dependencies, including path and cancellation. Only tools, packages, credentials,
detection, the application, the daemon, the toolkit and the terminal adapter may consume it, on native targets. It owns child-process lifecycles through its `child_process` module
(captured spawn, raw pipe collection, exit and EOF observation, termination and
reaping, with a caller-selected post-exit grace and TERM-then-KILL escalation); callers keep cancellation, termination, settlement and output policy.
Linux pane terminals use the separate `pty` module, with setup, foreground facts
and an owned I/O actor. The `byte_queue` module owns ordered buffers and partial-write
offsets shared by that actor and native terminal output; callers keep readiness and
shutdown policy. See [pane processes](workspace-daemon.md#pane-processes).

The listing leaf `maestro-listing` is declared core below layer 0 with no workspace
dependencies, including `maestro-path`. It owns directory listing: one directory's
entries, sorted by file name on every platform. Only resources, theme, the toolkit,
tools and session may depend on it, on native targets; they keep kind filtering,
name decoding and any further ordering. Guest-authoring examples may use it through
a portable development dependency from `maestro-extensions-wasm`, never a normal
or build dependency.
Synchronous capture follows the timeout handling in the
[contract](specs/maestro-port.md#crates-and-delivery-order). The checker verifies
dependency ownership, not target predicates.

The listener leaf `maestro-listeners` is declared core below layer 0 with no
workspace dependencies. It owns one job: retaining callbacks in live registration
order, admitting one per identity and returning removed ownership to the caller.
Models, the toolkit, the agent, the application and the extension runtime may
depend on it and keep their own borrowing, locking, delivery and settlement
policy. Retired registrations are reclaimed once no traversal runs and at least
half the slots are retired, so removal costs expected constant work amortized over
the removals and clearing every registration costs work proportional to their
number. While a traversal runs, retired slots stay until it ends.

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
directory. `resolve_with` delegates to `resolve_with_flavor` using the target
flavor; callers can select either flavor explicitly with `resolve_with_flavor`.
Both resolve several operand groups from callbacks the caller supplies: it calls the drive callback once, and the
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

`maestro-dependencies` is the other named exception: every member may declare it,
and only as a non-optional normal dependency under
`cfg(not(target_arch = "wasm32"))`. It has no internal dependencies, is not listed
in the specification table, cannot satisfy a required direct dependency and is the
only crate whose direct `serde_json` entries bypass member inheritance.

`maestro-path` has no domain dependencies and is the optional utility exception to
leaf and exact-set rules, alongside the native feature unifier. Any crate in the
specification table may declare it as a normal or build dependency (optional and target-specific forms included)
without changing its layer, subject to the exclusions in the specification's
[crate graph](specs/maestro-port.md#crates-and-delivery-order).
Shared path helpers take supplied directory observations; `native_homedir` alone
reads the native home environment and account entry. They add no filesystem
service, directory cache, scheduler or workspace dependency. The edge is ignored when checking a crate's complete
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
The four message delivery records belong to `maestro-request`; `ForkPosition` and
`NavigateTreeOptions` belong to `maestro-entry`. Duplicate declarations in these
owners are rejected. Qualified uses and re-exports are not declarations. Duplicate tool declarations
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

The chat tests that drive the interactive frontend build their session,
credentials, catalog and resource loader in
`crates/maestro-chat/tests/footer_support/session.rs`. The conventions check rejects
a second construction of the session configuration anywhere in the chat crate's
test code, naming its file and line, so every chat test shares one setup, which
returns a session with its services. The rule covers every chat test source in any
directory, except the setup itself. Paths compare by component, so a native
separator changes nothing, and the shared token walk includes macro arguments,
closures and impl bodies.

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
Optional `futures_util::poll!` invocations are also rejected in those test scopes,
including qualified/raw paths, known imports and literal nested macro tokens.
Pin the future and call `std::future::Future::poll` once with
`std::task::Context::from_waker(std::task::Waker::noop())` instead; this needs no
optional dependency feature. Macro aliases use the same file-wide scope and
limits as timer aliases. Comments, literal contents and ordinary function calls
are not macro invocations; no compiler expansion is performed.

Conventions reject token-identical production function bodies with at least 30
interior tokens, including free functions, methods, trait defaults and nested
functions. Whitespace and comments are ignored; token spelling, literal contents
and delimiter kinds are preserved. Each diagnostic lists the token count and all
native file paths with one-based signature lines, including same-line sites.
Parsed `cfg(test)` scopes, test file layouts, the generated model catalog and the
exact generated guest binding file are excluded. Macro definitions are not
expanded, but authored invocation tokens inside functions are compared.
Unparsed fragments produce no candidates; compound configurations are not
evaluated. There is no baseline, allowlist or suppression attribute.

The shipped extension example tests share their document decoding and
registration release in `src/tests/example_support.rs`. That release also asserts
the names the caller expects the host to have admitted and that nothing stays
lent. A per-example test beside it that writes the decode or the release walk
inline is rejected with its file and line, wherever the copy sits: a bare call, a
macro argument, a closure or a method.

Conventions reject a platform temporary directory that migrated-crate test code
names itself. Test-owned code is a test file layout and every item inside an
inherited `cfg(test)` context, closures and macro arguments included; the
`temp_dir` spelling is rejected there as a call, as a value or inside macro
tokens, and its import is rejected anywhere in the crate, whatever the alias.
The check follows no bindings or data flow. Tests that observe the platform
directory, name an uncreated socket path under it or pass it as the owner's
parent read it through `maestro_test_directory::temporary_root`, and directory
creation and removal go through `Scratch`. The check covers the crates the
migration onto `maestro-test-directory` has reached, and each batch adds its
crates until the last one removes the scope. The diagnostic names
`TEST_DIRECTORY_CALLERS`, where a new crate asks for the owner.

Clippy's protected `redundant_clone` lint rejects unnecessary copies workspace-wide.
Hand-written documentation uses only `///` and `//!`; conventions reject block doc comments with their file and line, except in generated guest bindings at `maestro-extensions-wasm/src/bindings.rs`. Line documentation in that file is still checked for repetition; allowed block comments end the comparison run.
Conventions reject consecutive identical nonempty trimmed physical documentation lines within one contiguous run of same-style line documentation comments, retaining list and quote markers. An attribute, an ordinary comment or code ends the run. All Markdown code blocks (including nested list and block-quote fences) reset comparison and are ignored. Blank documentation lines are retained and do not reset comparison; owner/style changes do. Headings, rules and HTML lines are compared in their physical order like other non-code lines.
Under `crates/*/src`, files carrying `#![cfg(test)]` must be named `tests.rs` or be below a `tests` directory. A module named `tests` must carry `#[cfg(test)]`, every `#[cfg(test)]` module must be named `tests`, and test module declarations cannot carry `#[path]`. These layout conventions support production line counting; module declarations are validated throughout the syntax tree.
Duplicate assertions are a review judgement.

Conventions reject a second or foreign declaration of `MessageRendererCall`:
`maestro-extensions` declares it exactly once in `src/types/renderers.rs`. Chat
production syntax may not name the raw extension machinery: `MessageRenderer`,
`CallbackEmitter`, `EventBus`, `get_message_renderer` or `invoke_callback`.
Import declarations, imported aliases, every segment of a qualified path and
method calls also count. An import path is read whole: a prefix module, a nested
group, a renamed item and a glob that names a restricted item are each rejected.
The frontend uses the projected call and `get_message_renderer_call` instead.
Test layouts, `cfg(test)` scopes, ordinary comments and string literals are
exempt. The scan reads parsed syntax, so it does not expand macros, resolve
arbitrary re-exports or follow unparsed fragments.

The root manifest must forbid every protected quality lint; member manifests
are parsed as TOML and must set the boolean `lints.workspace` to `true`.
The only exception is the guest-authoring manifest's exact table below;
missing lint wiring is rejected for every member, including that guest.
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

The guest-authoring crate declares its own lint table: the workspace table
with `pedantic`, `too_many_arguments` and `excessive_nesting` lowered from
`forbid` to `deny`, because generated bindings cannot compile under `forbid`.
Only that crate's generated `bindings.rs` may carry a lint attribute, the single
allowance `clippy::same_length_and_capacity`. Every handwritten
top-level module, integration test and example restores the groups with
`forbid`, nested files inherit it from their parent module, and the crate root
stays wiring-only.

Every member retains `unsafe_code = "forbid"`.
The [terminal emulator](terminal-emulator.md) is pure Rust.

The workspace daemon is core design: the root always wires the daemon,
multiplexer. Required direct edges are root → application and multiplexer,
and multiplexer → application. The required root
edges must be non-optional normal dependencies active on native builds. Like the
CLI and chat, the multiplexer may use application, toolkit, terminal adapter and
theme; it may also use the protocol crate for frame records. The foundation utility
and native feature-unifier exceptions apply too.
Declared-but-absent core owners require no placeholder crates.

Browser geometry uses installed `js-sys` host calls rather than a handwritten
binding macro: `Reflect::get` catches method lookup failures, `Function::call0`
retains the element receiver and catches invocation failures. Unchecked handle
conversion admits rectangles from a second same-origin realm. Both failure
phases reach the existing host error and restoration path; lint levels and the
generated-guest exception above are unchanged.

The 27 browser-library roots are declared in `.github/ci.toml`'s `wasm_crates`,
including `maestro-path`. Check each declared library locally with:

```sh
CARGO_BUILD_JOBS=3 capped mise exec -- cargo clippy --no-deps -p <crate> --target wasm32-unknown-unknown --locked -- -D warnings
```

The current shared wasm job builds these roots; it does not yet run this lint
command. The [shared wasm-lint rollout](https://github.com/Orchestration-Maestro/maestro-rust-workflows/issues/73)
tracks that separate extension. Native strict checks remain unchanged.

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

Exact test selection belongs to `maestro-test-process`. The conventions checker
rejects Rust string tokens whose decoded value is `--exact` outside that exact
Cargo package, reporting the source path and first violating line. It scans all
loaded Rust sources, including macro tokens, disabled configurations and source
fragments. Comments, larger fixture strings, byte strings and C strings are not
selector tokens. This bounded rule does not expand macros, resolve constructed
strings, interpret data files or infer process launches; review still checks
those cases.

Every crate builds its integration tests as one program from
`tests/integration.rs`. Add new `tests/*.rs` files as unconditional out-of-line
modules in that root; support modules stay in their existing directories. The
conventions check covers every workspace member, new crates included: a member
with top-level integration-test sources needs `autotests = false` and exactly
one test target, the runnable `integration` target at `tests/integration.rs`.

Tool metadata has one generic record owner, `maestro-request::tool_info::ToolInfo<P>`.
Native callers use its default parameter-schema owner through models; guest authoring
keeps a `ToolInfo` alias over the same owner. Live callback storage
belongs to `maestro-listeners::Listeners<P>`, with one admission per identity by
default or one per registration as an event-emitter listener list keeps them.
Agent subscriptions, session listeners, extension error delivery, terminal input
listeners and native output-failure handlers reuse it. For shared `Rc` callbacks it
also owns subscribing, which returns the removal
operation, and the live walk in registration order. What each listener receives
and when it runs remain with the caller. The collection returns removed ownership
for release outside guards, and it reclaims retired registrations while no
traversal runs, so one removal and a complete clear both stay cheap.

Extension runner sessions and context catalog getters retain one direct shared
`ModelRegistry` holder; its internal synchronization stays catalog-owned.
The complete action runtime composes the existing activity/flag state.
Binding installs action and base-context operations before provider flushing;
command-context binding remains independent.


Shared extension event/result schemas and assembled prompt records belong to
`maestro-request`, specialized with existing native/guest operands. The declaration
scanner rejects duplicate schemas while allowing specialization aliases and
re-exports. One event inventory feeds author unions and guest adapter data.

The extension `ToolDefinition<P,D,S>` alias specializes the existing tools record
with its concrete context and local invocation. The ownership scanner permits
that alias in native registration; the application root keeps standalone defaults.

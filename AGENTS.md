# Working on Maestro

## Flow

Land a spec on `main` under `docs/specs/` first. Then use `/to-tickets`,
`/implement` (with `/tdd`, then `/code-review`), `/pr`, and `/retro`.
Issues link to the spec rather than copying it.
The current foundation contract is [the Maestro specification](docs/specs/maestro-port.md).
Deliver crate by crate; write each crate's tickets when that crate is reached.

## Checks

Install mise and rustup, enable mise on `PATH`, then run:

```sh
mise install
just setup
just ci
```

`just setup` installs pinned tools and prek hooks. `just check` runs formatting,
`cargo hakari verify`, Clippy with warnings denied, strict public rustdoc and the
workspace conventions on this checkout; the conventions checker's own suite runs
with the workspace tests.
`just test` runs workspace tests with cargo-nextest, then Cargo doctests after
success. `just ci` runs checks and both test phases. Development and test builds
use line-table debug information: backtraces retain file:line, but debugger
variable views are unavailable.
Rust is pinned in `rust-toolchain.toml` and installed by rustup, never mise.
Run `cargo hakari generate` after changing dependencies, their features or
`Cargo.lock`, and `cargo hakari manage-deps` after adding a member (see
[development](docs/development.md#feature-unification)).
Use `mise exec -- just <recipe>` if mise is not active in your shell.

The commit hook formats the code, re-stages the staged files and names the
selection, then runs the shared check recipe for the staged crates and every
workspace crate that depends on them. A staged workspace-wide input runs the full
workspace check instead: a manifest, the lockfile, `.cargo/`, the toolchain, the
justfile, Clippy or mise settings, the hook configuration, the workspace
inventory, CI files, root documentation or assets, or the conventions crate. The
hook runs the full conventions suite when a staged path is under
`crates/maestro-test-conventions/`, and the browser smoke check only when the
selection compiles the browser entry. Unformatted code is fixed, never rejected.
It also rejects merge-conflict markers, invalid TOML or YAML, and a secret in the
staged change. Commit messages need a conventional header and every line must be
at most 80 columns. `just check` and shared CI stay the full workspace checks.

The conventions check requires `workspace.dependencies.serde_json` to name
`serde_json` and explicitly enable `float_roundtrip`. Normal member dependencies,
including aliases, targets and optional entries, must inherit a workspace entry
that enables it. Dev/build declarations are outside member inheritance enforcement
and do not supply the required workspace feature; test feature unification can
otherwise hide missing normal precision.

## Code quality limits

`just check` enforces at most 5 parameters (at most 1 boolean), 60 lines per
function, cognitive complexity 15, nesting depth 4 and
500 code lines per file; doc comments are not counted. Test directories,
`tests.rs` and trailing test modules do not count against file length. Pedantic
Clippy lints are errors; production code may not use `unwrap`, `expect` or
`panic!`. Unsafe code is forbidden. Thresholds live in
`clippy.toml`; levels live in `[workspace.lints]` and crates inherit them with
`[lints] workspace = true`. Clippy also rejects the ambiguous JSON map `remove`; see
[quality checks](docs/architecture.md#quality-checks). Quality-lint allowances are forbidden apart from the generated-binding exception below.
`maestro-extensions-wasm` declares its own lint table instead of inheriting the workspace's:
the same table with `pedantic`, `too_many_arguments` and `excessive_nesting` at `deny`
rather than `forbid`, because its generated bindings cannot compile under `forbid`. Its
generated `src/bindings.rs` alone may carry a lint attribute, the single allowance
`clippy::same_length_and_capacity`. Every handwritten top-level module, test and example of
that crate restores the three groups with `forbid`, and the workspace conventions enforce it.
The protected `redundant_clone` lint rejects unnecessary copies.
Conventions reject consecutive repeated nonempty physical documentation lines
within one contiguous run of same-style line documentation comments. An attribute,
an ordinary comment or code ends the run. Markdown code blocks, even inside lists
or block quotes, are ignored. Whitespace is trimmed but list/quote markers remain;
blank documentation lines are retained and do not reset comparison, while code
blocks and owner/style changes do.
Under `crates/*/src`, `#![cfg(test)]` files must be named `tests.rs` or be below a
`tests` directory. Modules named `tests` require `#[cfg(test)]`, every
`#[cfg(test)]` module must be named `tests`, and test modules cannot use `#[path]`.
These layout conventions support production line counting.
Conventions reject exact duplicate production function bodies of at least 30
interior tokens, ignoring only whitespace and comments. Parsed test scopes, test
layouts and the existing generated catalog/binding sources are excluded; macro
expansions and unparsed fragments are not checked, and compound configurations
are not evaluated. There are no suppressions or baseline. See
[quality checks](docs/architecture.md#quality-checks) for matching and diagnostics.
Duplicate assertions are a review judgement.
Conventions reject a platform temporary directory that test code in a migrated
crate names itself through `std::env::temp_dir()`, as a call, a value, a macro
argument or an import under any alias; tests that observe it use
`maestro-test-directory::temporary_root`, and the diagnostic names
`TEST_DIRECTORY_CALLERS`, where a new crate asks for that owner.
Conventions reject a second or foreign `MessageRendererCall` declaration and raw
`MessageRenderer`, `CallbackEmitter`, `EventBus`, `get_message_renderer` or
`invoke_callback` identifiers in chat production syntax, whatever the import form:
prefix modules, nested groups, grouped `self` renames, a glob that names one,
imported aliases, every segment of a qualified path and method calls. Comments,
string literals and test scopes are exempt.

## Documentation

Every change has a clear conventional commit message (the squash commit)
describing user-visible behavior. Pull requests never edit `CHANGELOG.md`.
The changelog is generated from conventional commits at release time.

Every crate root needs `//!` docs and every item, private included, needs
`///` docs, enforced by Clippy.
`just check` denies rustdoc warnings and missing documentation.
Hand-written documentation uses only `///` and `//!`; generated guest bindings in
`maestro-extensions-wasm/src/bindings.rs` may use block doc comments. Their line
documentation is still checked for repetition; block comments end the comparison run.

All Rust comments describe the code, not planning material. Never include
numbered slices, specs, tasks or tickets, issue/pull-request references,
planning vocabulary or planning identifiers. Workspace conventions scan
comments in all member Rust files, reporting file and line; strings and
attributes are not comments. No allowlist or exceptions: reword a false
positive. Ordinary technical terms such as "the JSON-RPC specification",
"an async task", "step 1 of the parse", "UTF-16" and "S3-compatible" pass.

## Crates

Dependency descriptions below state domain edges; each also permits the
[native feature-unifier exception](docs/specs/maestro-port.md#native-feature-unifier).
This includes leaves, exact sets and the core-to-dedicated prohibition; the unifier itself has no workspace dependencies.

- One job per crate, describable in one sentence without "and".
- Dependencies point one way; core crates never use dedicated
  (non-core) crates as normal or build dependencies; the declared test-support
  development edges are permitted under their owners' target rules.
- `maestro-path` is the foundation utility for lexical path strings. It sits
  below layer 0 and depends on no workspace crate. Native normalization reuses its
  segment reducer without decoding path bytes or accessing the filesystem.
  Native crates may declare it as a production dependency without changing their
  layer, subject to the
  exclusions in the specification's [crate graph](docs/specs/maestro-port.md#crates-and-delivery-order).
  Shared path helpers take supplied directory observations
  and add no global environment, filesystem service, directory cache, scheduler
  or workspace dependency. Final-link traversal uses a supplied observation callback;
  the utility owns relative-target joining and the forty-link bound, while callers
  retain native link I/O and error interpretation.
- `maestro-text` and `maestro-json` are declared core leaves below layer 0 with
  no workspace dependencies. Text owns common text operations (whitespace,
  lexical comparison, markup escaping, CSS string serialization, locale construction, streaming UTF-8, terminal control cleaning, newline
  normalization shared with `maestro-tools`); JSON owns JSON
  boundary semantics (JSON Pointer token escaping, object acceptance, canonical keys, truth, selected-value display, compact and
  pretty text, JSONL line framing). `maestro-entry` and `maestro-tool` define session (including generic compaction results) and tool
  interchange records (entry includes `ForkPosition` and `NavigateTreeOptions`) and depend only on `maestro-request`; `maestro-brand`
  projects a brand pack and depends on text and JSON. Text, JSON, entry, tool and brand exist; add no `common`, `utils` or
  `shared-types` crate. `maestro-pattern` depends only on text. `maestro-markdown` owns native Markdown
  parsing, owned source records and cursor-time extended links, depending only on text.
  The toolkit consumes it; application re-exports it as `markdown`. The dedicated
  conventions checker may consume its code-block source ranges for documentation
  checks. Terminal normalization and formatting remain in the toolkit.
  Keep authored paths as strings and convert to `PathBuf` only at I/O.
- `maestro-dependencies` is the generated, API-free feature unifier (`cargo hakari`),
  dedicated, unpublished and without workspace dependencies. Members declare it only
  as a non-optional normal dependency under `cfg(not(target_arch = "wasm32"))`; it
  never supplies a required edge. The crate is exempt from member `serde_json`
  inheritance (lint inheritance and the workspace precision entry still apply).
  Never hand-edit its generated manifest section.
- `maestro-watch` and `maestro-lock` are declared core leaves below layer 0.
  Watch has no workspace dependencies; lock may use the optional path utility
  without changing its leaf status. Only theme/application
  may depend on watch; only settings/credentials/workspace/detection/session/application may depend on lock.
  Application uses maestro-lock: atomic file publication, on native targets only. Follow the
  [shared filesystem effect contract](docs/specs/maestro-port.md#crates-and-delivery-order):
  watch owns creation/close helpers, injected watch/timer interfaces and native
  notifications; lock owns bounded and waiting acquisition of caller-opened files and byte-oriented
  atomic replacement. Workspace owns snapshot serialization and link-following policy.
  Feature policies remain with their owners, including asynchronous credential
  acquisition. Add no common background runtime, scheduler or listener framework;
  existing theme paths may re-export moved types, not duplicate implementations.
  Land this contract before creating the leaves; size moves by net added
  production lines with matching moved/deleted counts, not claimed savings.
- `maestro-process` is a declared core leaf below layer 0 with no workspace
  dependencies, including the path and cancellation leaves. Only tools, packages,
  credentials, detection, the application, the daemon, the toolkit and the terminal adapter may depend on it, on native targets. It owns native child-process
  lifecycles; the callers keep cancellation, termination and settlement policy and
  all output handling. Its retained-front byte queue is shared by pane-process input
  and native terminal output; callers own readiness and lifecycle policy. It also owns
  native termination-signal delivery, their idempotent removal and the prepared
  default action, while each native mode keeps its own exit policy. Synchronous capture follows the timeout handling in
  [the spec](docs/specs/maestro-port.md#crates-and-delivery-order).
- `maestro-listing` is a declared core leaf below layer 0 with no workspace
  dependencies, including the path utility. It lists one directory's entries sorted
  by file name on every platform. Only resources, theme, the toolkit, tools,
  session and the application may depend on it, on native targets; callers keep
  kind filtering, name decoding and further ordering. Guest-authoring examples may
  also use it through a portable development dependency from `maestro-extensions-wasm`,
  never a normal or build dependency.
- `maestro-listeners` is a declared core leaf below layer 0 with no workspace
  dependencies. It retains callbacks in live registration order and admits one per
  identity, or one per registration as an event-emitter listener list keeps them.
  Removal returns ownership to the caller, and traversal sees the collection as it
  is. Models, the toolkit, agent, application, extensions, the
  terminal adapter and cancellation may depend on it; they keep their own
  borrowing, locking and settlement policy.
  Its storage reclaims retired registrations while no traversal runs.
- `maestro-cancellation` owns cooperative cancellation below layer 0. Its only
  internal dependency is `maestro-listeners`, for synchronous abort observers.
  Only models and the toolkit may depend directly on it.
  It uses Tokio `=1.53.3`, defaults off, with `sync` on all targets.
- `maestro-request` defines shared message and model-request records below layer 0:
  model, message, content, usage, stream and diagnostic data, plus `SourceInfo`
  and `Skill`, plus the command provenance records `SlashCommandInfo` and
  `SlashCommandSource`, re-exported by resources, application presentation data and
  guest authoring rather than redeclared.
  Shared invocation options (`ThinkingBudgets`, `CacheRetention`, `Transport`,
  `ProviderResponse`, `StreamOptions`, `ProviderStreamOptions`, `SimpleStreamOptions`
  and `StreamOptionsInput`) and the data-only `StreamOptionSlots` family belong here;
  executable operations
  remain with their native or guest owners.
  Shared application messages include bash execution, custom, branch summary and
  compaction summary records with their existing `Presence<T>` wrapper, plus the
  renderer option record `MessageRenderOptions`, `CustomMessageInput<T>`, `MessageDelivery`,
`UserMessageDelivery`, `SendMessageOptions` and `SendUserMessageOptions`, re-exported by session and guest
  authoring rather than redeclared.
  It also owns `model_registry::ResolvedRequestAuth`, re-exported by the native
  catalog and guest authoring library, the plain credential records
  `oauth::OAuthCredentials` and `auth_storage::{ApiKeyCredential, AuthCredential,
  AuthStorageData, AuthSource, AuthStatus}`, re-exported by models and credentials,
  and the provider registration records `provider_config::{ProviderConfig<C>,
  ProviderModelConfig}`, specialized by the catalog and the guest, default zero usage, message-only diagnostic
  construction and the shared optional-field helper. Only the crates that list it in the spec table
  (models, resources, theme, credentials, catalog, session, entry, tool and guest authoring)
  may depend on it; it depends only on the JSON leaf. Keep existing models/resources public paths as re-exports.
  Provider/runtime operations and resource discovery stay in their owners.
  The spec's shared-record amendment governs extraction; update the enforced
  table, inventory, graph policy and architecture tests together with the code.
- Names follow `maestro-<noun>[-<role>]`.
- When present, the composition-root crate owns the binary and only wires.

Native footer repository discovery uses the workspace git owner through a
native-only `maestro-app` to `maestro-workspace` edge. A typed policy retains
caller-specific metadata rules within the one shared walk. Browser builds have
no native repository; footer statuses, counts and callbacks remain available.

The read-only footer contract, its live status view and the generic live map behind
them belong to `maestro-extensions::footer_data_provider`; the application footer
owner keeps its policy and re-exports them at its existing paths.

Browser page content (`maestro-web::dom`) and runtime contributions
(`maestro-web::contributions`) stay inside the web frontend. Contribution records
belong to `maestro-protocol::contributions` and are re-exported by
`maestro-app::contributions`, whose native reader loads the enabled files the
package manager resolves as its fifth resource kind. `custom-element =0.1.5` is the
browser-only element bridge; see [browser libraries](docs/libraries/browser-frontend.md).

Pure retained session-tree ordering, geometry, content extraction/search and raw-record ancestry belong to `maestro-session`,
re-exported through `maestro-app`; frontend filter, fold and selection state stay
with their frontend.

The guest authoring crate may depend on the request, entry, tool and JSON
owners and never imports native session, tool or catalog runtimes. The terminal
adapter requires the toolkit and `maestro-text` as direct dependencies; `maestro-text` is
created, and so is `maestro-json`; the other declared leaf owners stay policy rows until their
extraction issues.

Conventions enforce the [foundation graph](docs/specs/maestro-port.md#crates-and-delivery-order)
through declared and host-resolved Cargo metadata, including optional, target and
build edges. Classes are distinct from delivery layers: only the binary root,
conventions checker, terminal scenario harness, checked test-process owner,
disposable test-directory owner, test tracing owner, portable fixture owner,
repository tooling and the feature unifier (`maestro-dependencies`) are dedicated.
Repository tooling (`maestro-tooling`) is development-only, never shipped and has
no internal dependencies apart from the optional utility and the unifier. Sparse workspaces need
no placeholder crates. Frontends, including the multiplexer, require the application edge and may declare
only the other edges in their allowed sets, when used. The terminal adapter
requires toolkit and text, and the scenario harness requires toolkit only. The
utility edge never supplies a required edge. The daemon requires the terminal
engine. Other non-leaves permit subsets.
Production and test graphs are separately acyclic. The internal development
edges are `maestro-tui-crossterm` to `maestro-brand`, for terminal examples,
the native-only `maestro-extensions` to `maestro-credentials`, for extension tests
that build a real model catalog, the native-only `maestro-extensions-wasmtime`
to catalog, credentials, models, session, theme and toolkit for reused native composition
test support, including a real terminal for synchronous component factories, the native-only, non-optional `maestro-detection`
to `maestro-text` edge for terminal newline normalization in checked-child tests,
the native `maestro-rpc` to `maestro-theme` and `maestro-tui-crossterm` edges
for its standard-stream child tests, the native-only, non-optional `maestro-chat`
to `maestro-models` edge for interactive-mode tests that drive a real session
through the scripted provider, the native-only, non-optional `maestro-cli`
to `maestro-listeners` development edge for the controlled print adapter,
the native-only, non-optional `maestro-extensions-wasm` to `maestro-models`
development edge for component tests of the native schema validator,
the native-only, non-optional `maestro-extensions-wasm` to `maestro-extensions-wasmtime`
development edge for the product's one engine configuration,
and the native, non-optional dev-dependencies
of the twenty-six declared callers on `maestro-test-process`, the owner of
checked child tests (see
[the spec](docs/specs/maestro-port.md#test-process-owner)); a caller declares it
only when it uses it. Brand and theme remain forbidden as adapter normal/build
dependencies, credentials as an extension one; the brand edge never satisfies
the required toolkit/text set. The scenario harness's recording terminal and
manual runtime are the one shared test-effect owner, declared as a native
development dependency by `maestro-cli` only.

The dedicated `maestro-test-directory` native leaf owns exclusive disposable-root
creation and cleanup. It has no internal edge except the native unifier, and no
foundation utility permission. Only the crates its [directory
contract](docs/specs/maestro-port.md#test-directory-owner) names may declare it,
as a non-optional native development dependency when used. `Scratch::new`
canonicalizes its chosen root, without the Windows verbatim drive prefix;
`Scratch::new_in` keeps native supplied-parent spelling. On Unix, `enter_removed`
and `enter_unreadable` put a test process in a removed or unreadable working
directory, and `deny(path, mode)` sets a mode and
returns its restoring guard, or `None` when the process is not refused, as in a
privileged run. `keeps_distinct(names)` reports whether the running host keeps a set of
leaf names apart, and `folded_names(paths)` reports every name group it does not keep
apart across all components of the supplied relative paths, so a fixture can state a
premise the host can exercise.
Both graph cycle checks still apply.

The dedicated, unpublished `maestro-test-fixtures` portable leaf owns shared test
fixture operations. Only agent, models, request, catalog, chat, credentials, tools
and export may declare it as a development dependency, portable or target-specific;
models, request, catalog, chat and credentials use portable declarations, while
agent, tools and export use native declarations. It has no product or utility
edge, only the native unifier exception. Both cycle checks apply. See the
[fixture contract](docs/specs/maestro-port.md#test-fixture-owner).

The dedicated `maestro-test-tracing` native leaf runs a test action under a
scoped tracing subscriber that sees every callsite. Only daemon, models, process,
terminal and workspace may declare it, under the directory owner's rules. Tests
capture through its `with_default` or, for a future, its `with_subscriber`. A
conventions check rejects `tracing`'s scoped subscriber functions and the
`with_subscriber` method in other test sources. Formatting subscribers write into
its `Output` sink. See the
[tracing contract](docs/specs/maestro-port.md#test-tracing-owner).

The guest may declare `ToolDefinition`, `ToolRenderContext` and `ToolRenderResultOptions`
only in `src/types/tools.rs`, in addition to their domain owner. Native extensions may
specialize `ToolDefinition` with the alias in `src/types/registration.rs`; elsewhere the
renderer declarations retain their domain ownership.

Bounded source/build checks enforce tool/selector ownership, direct runtime-engine
and toolkit library placement, canonical guest-owned WIT inputs and planning-comment
policy. They ignore literals/comments when finding declarations. Dynamic WIT inputs
require review and never count as verified. Manual review still proves generic
caller context, wiring-only binaries, no frontend application policy, complete
code-generation evidence and framework/highlighting equivalents. See
[architecture checks](docs/architecture.md) for commands, diagnostics and limits.
Publish reusable adapter conformance from the owning leaf, not a forwarding test crate.

`maestro-protocol` owns the generic RPC envelopes, the workspace records, binary
screen channel and generated JSON Schema, with no domain dependencies.
`maestro-app::rpc_types` owns the session records and `maestro-app::rpc_mode` the one
connection runner; `maestro-rpc` is its standard-stream adapter. Session and workspace
operations use one contract across stdio and daemon sockets; JSONL framing belongs to
`maestro-json::jsonl`. Conventions keep protocol records in the application and the
protocol crate. The connection output accepts only an `RpcLine`, which only an
`RpcRecord` builds, so protocol output has one writer by type. Native input, rendering and notification conversions stay with their runtime adapters. See
[daemon protocol](docs/daemon-protocol.md).

The workspace daemon is core design: the root always wires the daemon through
the application interface. A direct multiplexer dependency is permitted when its
launcher is wired, not required for command-line startup.
These declared owners need no placeholder crates before their first delivery.

## Libraries and formats

The base owner-approved libraries are: tokio, reqwest, serde, serde_json,
toml, clap, rmcp, tracing, and globset. Use Git through the `git` command.
Ask before adding any unapproved crate; never add one silently.

`maestro-models` pins `reqwest =0.13.5` (default features plus `stream`),
`url =2.5.8`, `futures-core =0.3.34` (`std`) and `futures-util =0.3.34`
(`std` and `sink`),
`httpdate =1.0.3` and `ryu-js =1.0.3`. Native targets add `tokio =1.53.3`
(`rt`, `time`; `test-util` for tests); browser targets add `wasm-bindgen =0.2.129`,
`wasm-bindgen-futures =0.4.79` and `js-sys =0.3.106`. All are MIT OR Apache-2.0
except tokio (MIT) and ryu-js (Apache-2.0 OR BSL-1.0, used under Apache-2.0).
The models crate reads server-sent events with its own reader, so no event-stream
library is a dependency.
Native proxy scripts use `boa_engine =0.22.0` (defaults off, `system-time-zone`,
MIT), `tracing =0.1.44` (defaults off, `std`, MIT) for authored alerts, and
`if-addrs =0.15.0` (defaults off, `link-local`, MIT OR BSD-3-Clause) for the
local IPv4 fallback. FTP script sources use Maestro's own native client over the
locked Tokio, regex and chrono facilities instead of an FTP dependency. These
dependencies do not enter browser targets.
Native SOCKS connections use `tokio-socks =0.5.3` (MIT, defaults off, `tokio`) and its
`thiserror =1.0.69` closure, beside reqwest. Native connector support directly reuses locked
`hyper =1.11.1`, `hyper-util =0.1.21`, `hyper-rustls =0.27.10`, `rustls =0.23.45`,
`rustls-platform-verifier =0.7.1` and `tower-service =0.3.3` with the features listed in
[provider qualification libraries](docs/libraries/provider-qualification.md). Request/response
conversion and TLS setup stay single-owned; SOCKS request futures own their connection work.
Native targets of `maestro-models` also use `aws-sdk-bedrockruntime =1.148.0` and
`aws-config =1.12.0` for Bedrock credentials, signing, retries and event framing, with
`aws-smithy-runtime-api =1.19.0` and `aws-smithy-types =1.8.1` for the transport bridge,
`aws-credential-types =1.3.0` for the typed credential errors of the selected-profile sources, and
`aws-sdk-signin =1.22.0` for the typed login-refresh error (`aws-config` enables its
`credentials-login` feature)
(see [provider qualification libraries](docs/libraries/provider-qualification.md)); the browser
build contains no AWS crate.

Keep one current format for everything. No compatibility code.

Extension event payloads and results use plain JSON with serde_json semantics.
Finite numbers roundtrip exactly, including signed zero; ordinary host
serialization maps nonfinite numbers to null. Reject an extension-written
Infinity or NaN with a clear error before encoding the edited event or result.
Use shared records directly, without bit-encoded numbers, numeric transport
wrappers or added Presence wrappers. Preserve existing shared optional-field
semantics and the missing/null distinctions of unshared event fields.

Browser author recipes and the standalone editable fixture are documented in
[the browser author guide](docs/browser-build.md). The fixture is not a workspace
member or shipped application. Whole shared fonts retain their family notices;
external author tools stay in mise, not product dependencies or install hooks.

## Public text

Describe Maestro in its own words. Name no other agent tool.
Follow the [identity guide](docs/identity.md) for voice, code naming and brand
assets. Consumers read brand values from the pack, never hard-code them.
Before changing a themed surface, read [PRODUCT.md](PRODUCT.md),
[DESIGN.md](DESIGN.md), [docs/identity.md](docs/identity.md) and
[assets/brand/brand.json](assets/brand/brand.json).

## Agent skills

[Development prompts](docs/development_prompts.md) cover release audits, issue
analysis, PR review and task completion through the existing repository process;
the prompt files live in `.maestro/prompts/`.

### Issue tracker

Issues are GitHub Issues in `Orchestration-Maestro/maestro`; specs land on
`main` first. See `docs/agents/issue-tracker.md`.

### Triage labels

Matt Pocock's five default triage labels. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context, with the glossary in `CONTEXT.md` and ADRs in `docs/adr/`.
See `docs/agents/domain.md`.

## Contribution policy and communication

Read [CONTRIBUTING.md](CONTRIBUTING.md). Reports below its quality bar need not
receive a reply. Human triage remains final.
Use the existing five triage-role mappings, not a parallel package-label list.

Keep communication concise, technical and concrete, without emojis or filler.
Check external API types and documentation rather than guessing. Prefer
ordinary top-level imports and clear Rust types. Ask before removing intentional functionality; dependency errors do not justify downgrading behavior. Keep
configurable keybindings with registered defaults, never hard-coded checks.
The generated catalog belongs to its generator: change generation inputs or
logic, never edit generated model records directly. Its owning module declaration
skips rustfmt so generated source stays byte-for-byte as emitted.
Native developer feed APIs are explicit operations separate from runtime catalog
lookup; they return descriptors without registering models or writing catalog files.
Explicit `just models-generate`, `models-build` and workspace `build` acquire
feeds; ordinary Cargo builds, checks, tests and watches remain offline. See
[catalog generation](docs/models/generation.md).

For issue/PR comments, write the complete text to a temporary file, use
`gh issue comment --body-file` or `gh pr comment --body-file`, and Preview
exact text before posting. Post one final concise comment unless asked for more.
Delete a malformed comment before posting its single correction. Include
`Closes #<number>` or `Fixes #<number>` for related issue-closing commits.

## Test and Git discipline

`just check` is distinct from `just test`: read the full output, fix warnings,
and run each changed test file plus focused cases. Use deterministic controlled
provider fixtures, never live provider APIs, real keys or paid tokens by default.
As #109 specifies, `just test` remains unwrapped; the separate ported non-LLM test
script removes the specified provider credentials and sets the agent's auth file
aside, restoring it on exit. Name regression cases after behavior without leading
issue numbers.

Analyze remote PR metadata before checking it out. Commit only when authorized.
Use a feature branch, signed `git commit -S`, protected PRs and the merge queue;
never directly push to the default branch or bypass hooks/protection.

Stage only your own session files with `git add <specific-file-paths>`.
Inspect `git status` and the staged diff before committing. Never use
`git add .`, `git add -A`, `git reset --hard`, `git checkout .`, `git clean -fd`,
blanket `git stash`, or `git commit --no-verify`. Rebase on current main;
retain its behavior and assertions. Resolve conflicts only in owned files;
abort and ask if an unrelated file conflicts. Force-push only with explicit
standing approval and a fetched-hash lease. Ask for confirmation before an
instruction overrides these safeguards.

## Provider contribution checklist

This checklist governs contributions as each owning interface is delivered;
it does not claim an absent product surface is implemented.

1. Add the protocol/provider identity and options through the owning registration
   interface. Keep vocabulary extensible. Define provider-specific stream options
   and their common-option mapping. Implement invocation, message/tool conversion
   and standardized text, thinking, tool-call, usage and stop events.
2. Export the intended provider surface and option types. Keep lazy registration
   separate from implementation loading. Add credential detection and nonstandard
   authentication utilities; do not expose third-party library types.
3. Update generated catalog acquisition/parsing and map descriptors into the
   owning Model interface. Document defaults, display names, environment variables,
   setup instructions and authentication configuration in the relevant feature
   docs and providers table.
4. Add a representative stream model even for a reused protocol. Exercise broader
   token counting/total usage, abort, empty response, context overflow, image
   limits, Unicode, missing tool result, image tool result and cross-provider
   handoff cases. Include a provider/model pair per model family where applicable;
   use controlled credentials and deterministic protocol fixtures.
5. Document public options, authentication, exports and setup. Run `just check`
   and `just test`; history remains release-generated, not hand-edited.

## Controlled terminal walkthrough

The terminal frontend/tool is not implemented yet. Once delivered, use its
published launch command in a local disposable tmux session, not a live provider
session. Create an 80 by 24 terminal, launch the delivered command, capture startup,
then send a controlled prompt and special keys. Do not infer startup readiness
from a fixed sleep. Always clean up the session:

```sh
tmux new-session -d -s maestro-test -x 80 -y 24
# Send the delivered terminal launch command followed by Enter.
tmux capture-pane -t maestro-test -p
tmux send-keys -t maestro-test 'controlled fixture prompt' Enter
tmux send-keys -t maestro-test Escape
tmux send-keys -t maestro-test C-o
tmux capture-pane -t maestro-test -p
tmux kill-session -t maestro-test
```

## Release contribution rules

Keep lockstep package versions. A patch release includes fixes and new features;
a minor release includes breaking API changes. Release preparation synchronizes
versions, finalizes generated notes, prepares a conventional commit/tag and
packages artifacts; separately authorized publication publishes them. Do not
invent a publisher command before that tooling is delivered. Released history
is immutable; release automation retains change attribution and creates the
next release's history from conventional commits. PRs never edit CHANGELOG.md.

Approved libraries live in [docs/libraries/](docs/libraries/), one file per area.
A ticket that adds a library adds or edits only its area's file.

Tests wait for completion signals; nextest bounds each test, and shared CI bounds
the job.
A test child that must answer a signal blocks it and takes it with `sigwait`:
a handler can miss a signal that arrives just before a blocking call, and the
test then waits forever.
Conventions reject recognized test-owned timer calls (Tokio `timeout`/`timeout_at`,
standard timed receives, condition-variable waits and thread parking), including
file-level import aliases. Production deadlines remain allowed. The scan does not
model local shadowing, expand macros, infer receiver types, resolve arbitrary
re-exports/function values or evaluate compound configurations; those forms and
sleep-based watchdogs require source review.
Optional `futures_util::poll!` invocations are also rejected in those test scopes,
including qualified/raw paths, known imports and literal nested macro tokens.
Pin the future and call `std::future::Future::poll` once with
`std::task::Context::from_waker(std::task::Waker::noop())` instead; this needs no
optional dependency feature. Macro aliases use the same file-wide scope and
limits as timer aliases. Comments, literal contents and ordinary function calls
are not macro invocations; no compiler expansion is performed.

Every crate builds its integration tests as one program from
`tests/integration.rs`. Add new `tests/*.rs` files as unconditional out-of-line
modules in that root; support modules stay in their existing directories. The
conventions check covers every workspace member, new crates included: a member
with top-level integration-test sources needs `autotests = false` and exactly
one test target, the runnable `integration` target at `tests/integration.rs`.

Tool parameter schemas have one owner, `maestro-request::parameter_schema::ParameterSchema<C = ()>`,
which keeps the supplied JSON Schema value beside the kinds its author declared for
individual nodes and an optional executable payload. Shared Base/refinement records
keep native callbacks local; the guest specializes the same records with borrowed
JSON views and its existing callback resources. Ordinary serialization omits code. `Tool`, `Context` and `ToolInfo` default to that owner, the guest
re-exports it, and the model argument-validation owner is its only converter.
Serialization writes the supplied JSON alone; provider payloads and saved JSON never
carry the declared kinds.

Tool metadata has one generic record owner, `maestro-request::tool_info::ToolInfo<P>`.
Native callers use the default parameter-schema owner through models; guest authoring
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


Shared extension event/result schemas and assembled prompt records belong to
`maestro-request`, re-exported through models and specialized by native/guest
aliases. One event inventory feeds author unions and the guest data adapter.
Request depends only on the JSON leaf; object reading stays JSON-owned and
Presence adaptation stays beside shared schemas. Fork position stays entry-owned.
Tool callback ownership and invocation are independent: SDK defaults remain
Threaded/Threaded, registered invocation is Local. Registration stores no runner
or context factory; the single tools wrapper creates a fresh context per call.

The native extension `ToolDefinition<P,D,S>` alias specializes the tools-owned
record with ExtensionContext and local invocation; the application keeps its
standalone default alias. The declaration checker permits only this native alias
at `maestro-extensions/src/types/registration.rs`, never a second struct or enum.

The parameter-schema owner also owns the component metadata transport in
`maestro-request::parameter_schema::transport`, re-exported through models and native extensions
as `schema_transport`. Guest registration and enumeration use that same codec;
ordinary provider and persisted JSON omit the carried schema kinds. This adds no
crate dependency: both extension adapters already reach the request owner through
their declared edges.

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
Clippy with warnings denied, strict public rustdoc and workspace conventions.
`just test` runs all tests; `just ci` runs both, matching shared Linux CI.
Rust is pinned in `rust-toolchain.toml` and installed by rustup, never mise.
Use `mise exec -- just <recipe>` if mise is not active in your shell.

The commit hook formats the code, re-stages the staged files and runs
`just check`; unformatted code is fixed, never rejected. It also rejects
merge-conflict markers and invalid TOML or YAML. Commit messages need a
conventional header and every line must be at most 80 columns.

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
`[lints] workspace = true`. Quality-lint allowances are forbidden, with one exception.
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
Duplicate assertions are a review judgement.

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

- One job per crate, describable in one sentence without "and".
- Dependencies point one way; core crates never depend on dedicated
  (non-core) crates.
- `maestro-path` is the foundation utility for lexical path strings. It sits
  below layer 0 and depends on no workspace crate. Native crates may declare it
  as a production dependency without changing their layer; the guest authoring
  crate, the component runtime adapter, the shared request-record crate and the
  terminal scenario harness, cancellation leaf, watch leaf and lock leaf may not.
  Keep authored paths as strings and convert to `PathBuf` only at I/O.
- `maestro-watch` and `maestro-lock` are declared core leaves below layer 0 with
  no workspace dependencies, including the path utility. Only theme/application
  may depend on watch; only settings/credentials may depend on lock. Follow the
  [shared filesystem effect contract](docs/specs/maestro-port.md#crates-and-delivery-order):
  watch owns creation/close helpers, injected watch/timer interfaces and native
  notifications; lock owns bounded synchronous acquisition of caller-opened files.
  Feature policies remain with their owners, including asynchronous credential
  acquisition. Add no common background runtime, scheduler or listener framework;
  existing theme paths may re-export moved types, not duplicate implementations.
  Land this contract before creating the leaves; size moves by net added
  production lines with matching moved/deleted counts, not claimed savings.
- `maestro-cancellation` owns cooperative cancellation below layer 0, with no
  internal dependencies. Only models and the toolkit may depend directly on it.
  It uses Tokio `=1.53.2`, defaults off, with `sync` on all targets.
- `maestro-request` owns model-request records below layer 0: model, message,
  content, usage, stream and diagnostic data, plus `SourceInfo` and `Skill`.
  Only models, resources, theme and guest authoring may depend on it; it has no internal
  dependencies. Keep existing models/resources public paths as re-exports.
  Provider/runtime operations and resource discovery stay in their owners.
  The spec's shared-record amendment governs extraction; update the enforced
  table, inventory, graph policy and architecture tests together with the code.
- Names follow `maestro-<noun>[-<role>]`.
- When present, the composition-root crate owns the binary and only wires.

Conventions enforce the [foundation graph](docs/specs/maestro-port.md#crates-and-delivery-order)
through declared and host-resolved Cargo metadata, including optional, target and
build edges. Classes are distinct from delivery layers: only the binary root,
conventions checker, terminal scenario harness and repository tooling are dedicated.
Repository tooling (`maestro-tooling`) is development-only, never shipped and has
no internal dependencies apart from the optional utility. Sparse workspaces need
no placeholder crates. Frontends require their full direct sets; the terminal
adapter and scenario harness require toolkit only. The utility edge never counts
toward an exact set. Other non-leaves permit subsets.
Production and test graphs are separately acyclic; the internal dev-target
allowlist is empty. The scenario harness is not a general dev-dependency target.

The guest may declare `ToolDefinition` only in `src/types/tools.rs`, in addition to
its domain owner; renderer declarations retain their domain ownership.

Bounded source/build checks enforce tool/selector ownership, direct runtime-engine
and toolkit library placement, canonical guest-owned WIT inputs and planning-comment
policy. They ignore literals/comments when finding declarations. Dynamic WIT inputs
require review and never count as verified. Manual review still proves generic
caller context, wiring-only binaries, no frontend application policy, complete
code-generation evidence and framework/highlighting equivalents. See
[architecture checks](docs/architecture.md) for commands, diagnostics and limits.
Publish reusable adapter conformance from the owning leaf, not a forwarding test crate.

## Libraries and formats

The base owner-approved libraries are: tokio, reqwest, serde, serde_json,
toml, clap, rmcp, tracing, and globset. Use Git through the `git` command.
Ask before adding any unapproved crate; never add one silently.

`maestro-models` pins `reqwest =0.13.5` (default features plus `stream`),
`url =2.5.8`, `futures-core =0.3.34` (`std`) and `futures-util =0.3.34`
(`std` and `sink`),
`httpdate =1.0.3` and `ryu-js =1.0.3`. Native targets add `tokio =1.53.2`
(`rt`, `time`; `test-util` for tests); browser targets add `wasm-bindgen =0.2.129`,
`wasm-bindgen-futures =0.4.79` and `js-sys =0.3.106`. All are MIT OR Apache-2.0
except tokio (MIT) and ryu-js (Apache-2.0 OR BSL-1.0, used under Apache-2.0).
The models crate reads server-sent events with its own reader, so no event-stream
library is a dependency.

Keep one current format for everything. No compatibility code.

Extension event payloads and results use plain JSON with serde_json semantics.
Finite numbers roundtrip exactly, including signed zero; ordinary host
serialization maps nonfinite numbers to null. Reject an extension-written
Infinity or NaN with a clear error before encoding the edited event or result.
Use shared records directly, without bit-encoded numbers, numeric transport
wrappers or added Presence wrappers. Preserve existing shared optional-field
semantics and the missing/null distinctions of unshared event fields.

## Public text

Describe Maestro in its own words. Name no other agent tool.
Follow the [identity guide](docs/identity.md) for voice, code naming and brand
assets. Consumers read brand values from the pack, never hard-code them.

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

## Approved repository-tooling libraries

The private conventions checker may use `syn =3.0.6` (defaults off;
`full`, `parsing`, `printing`, `visit`), `proc-macro2 =1.0.107` (default
`proc-macro` plus `span-locations`), `toml =1.1.6` (locked as
`1.1.6+spec-1.1.0`; default `std`, `serde`, `parse`, `display`),
`ra-ap-rustc_lexer =0.176.0` (no features) and `cargo_metadata =0.23.1`
(defaults off; no features). All are MIT OR Apache-2.0 except
`cargo_metadata`, which is MIT. Dependency feature unification also enables
syn's defaults (`derive`, `parsing`, `printing`, `clone-impls`, `proc-macro`).
`unicode-ident` is held at 1.0.24 in the lockfile because the lexer asserts
matching Unicode tables; the hold lifts when `unicode-properties` publishes
the newer tables.

Documentation parsing and the toolkit Markdown component use
`pulldown-cmark =0.13.4` with defaults off (MIT). The toolkit enables only
strikethrough and task-list parsing; task markers do not add checkbox rendering.
Its additional dependencies are `bitflags =2.13.2` and `unicase =2.10.0`
(MIT OR Apache-2.0).

`maestro-tooling` alone may use `cargo_metadata =0.23.1` with default features
(MIT). Its dependency closure uses MIT OR Apache-2.0 (camino, cargo-platform,
semver, serde, serde_core, serde_derive, serde_json, itoa, proc-macro2, quote,
syn, thiserror and thiserror-impl), Unlicense OR MIT (memchr),
MIT (zmij), and (MIT OR Apache-2.0) AND Unicode-3.0 (unicode-ident).

## Approved resource parsing libraries

The resources crate uses `yaml-rust2 =0.13.0` with defaults off for native
YAML parsing events, pinned to the patched copy in `vendor/yaml-rust2` until
[#381](https://github.com/Orchestration-Maestro/maestro/issues/381) brings the
correction into an upstream release, and `ryu-js =1.0.3` with defaults for numeric scalar-key
spelling. Their selected license branches are MIT OR Apache-2.0 and
Apache-2.0, respectively. Discovery uses `ignore =0.4.23` with defaults off for
compiled ignore rules. Authored path operations use the shared `maestro-path`
utility. Radix scalar resolution uses `num-bigint =0.4.8` and
`num-traits =0.2.19` with defaults (MIT OR Apache-2.0) for arbitrary-width integers
and correctly rounded floating-point conversion. Resources may depend internally
only on the shared request-record owner and the shared path utility.

## Approved scripted-model libraries

The models crate uses rand `=0.10.3` with defaults. Native scheduling uses
Tokio `=1.53.2`, defaults off, with `rt-multi-thread`, `sync` and `time`.
Browser scheduling uses js-sys `=0.3.106`, wasm-bindgen `=0.2.129` and
wasm-bindgen-futures `=0.4.79` with defaults; getrandom `=0.4.3` enables
`wasm_js`. Compact serde_json `=1.0.151` retains `preserve_order` and the
workspace-wide `float_roundtrip`; the models crate adds `raw_value` to keep
number syntax as written, and `arbitrary_precision` stays off because it changes
every `Number`. These MIT/Apache-2.0 libraries add no internal crate edge or
public runtime API.

## Approved toolkit libraries

`maestro-tui` alone may use `unicode-segmentation =1.13.3` and
`unicode-width =0.2.2` (both with defaults off) for grapheme boundaries and
scalar cell widths, and `regress =0.12.0` (defaults `backend-pikevm` and `std`,
plus `prohibit-unsafe`) for the Unicode property expressions of its width policy.
All three are MIT OR Apache-2.0. For the text helpers, the only added transitive
dependency is
`memchr =2.8.3` (Unlicense OR MIT; `alloc` and `std`). No type of these libraries
appears in a public interface.

For inline images, `maestro-tui` may use `base64 =0.23.1` (defaults off,
`std`) and `rand =0.10.3` (defaults). Its browser target declares
`getrandom =0.4.3` with `wasm_js` independently. These libraries are MIT OR
Apache-2.0; base64 adds no transitive dependencies, and the existing random/browser
closure is reused. Fixture consumption uses `serde_json` only as a dev-dependency.
No third-party type appears in the image interface.

## Approved frame-diagnostics and scenario libraries

`maestro-tui` may use `serde =1.0.229` (defaults, `derive`) and the workspace
`serde_json =1.0.151` (defaults, `preserve_order`) to serialize its typed
diagnostic records; both are MIT OR Apache-2.0 and no type of either appears in a
public interface. The added closure is `serde_core 1.0.229`, `serde_derive 1.0.229`,
`indexmap 2.14.2`, `equivalent 1.0.2`, `hashbrown 0.17.1`, `itoa 1.0.18`,
`zmij 1.0.23` (MIT), `proc-macro2 1.0.107`, `quote 1.0.47`, `syn 3.0.6` and
`unicode-ident 1.0.24`, which keeps its lockfile hold.

`maestro-test-terminal` alone may use `vt100 =0.16.2` (MIT, default features) as a
development dependency to replay the frame writer's bytes into an emulated screen,
with dev-only `serde =1.0.229` and `serde_json` as above to read typed fixtures. The
emulator's closure is `vte 0.15.0` (`default`, `std`; Apache-2.0 OR MIT),
`arrayvec 0.7.8` (MIT OR Apache-2.0), `itoa 1.0.18`, `memchr 2.8.3` (`alloc`, `std`;
choose MIT) and `unicode-width 0.2.2` (`default`, `cjk`). The emulator never enters
a product crate, and no crate depends on the harness.

## Approved tool-argument libraries

`maestro-models` may use the following exact native primitive pins for argument
checking, without an internal crate edge or an additional public schema API:

- `num-bigint =0.4.8`, `num-traits =0.2.19`,
  `unicode-segmentation =1.13.3`, `regress =0.12.0`, `url =2.5.8`,
  `percent-encoding =2.3.2` and `idna =1.1.0`, with their default features;
  all are MIT OR Apache-2.0.
- `ryu-js =1.0.3`, with defaults, Apache-2.0 OR BSL-1.0; use Apache-2.0.
- `chrono =0.4.45`, defaults off, only `std`, MIT OR Apache-2.0.
- `icu_normalizer =2.3.0`, defaults off, only `compiled_data`, Unicode-3.0.
  This direct dependency is already in the IDNA closure and supplies NFC without
  replacing the case-sensitive hostname context checks.

The IDNA dependency closure also contains Unicode-3.0 ICU components. Keep every
pin exact and inspect the resolved licence and feature closure after changes.

## Approved settings libraries

The settings crate uses serde `=1.0.229` with `derive`, serde_json `=1.0.151`
with `preserve_order` and `float_roundtrip` (exact decimal parsing, enabled once
on the workspace dependency for every crate), Tokio `=1.53.2` with defaults off
and `sync` for completion barriers, and `ryu-js =1.0.3` for float spelling at
the JSON boundary (Apache-2.0 OR BSL-1.0).
Browser scheduling uses wasm-bindgen-futures `=0.4.79`. Native tests add Tokio's
`rt` feature. These add no internal crate edge or public runtime API.

## Approved OAuth primitive libraries

`maestro-models` uses base64 `=0.23.1` (defaults off, `std`), sha2 `=0.11.0`
(defaults off) and getrandom `=0.4.3` (defaults; browser `wasm_js` retained).
All are MIT OR Apache-2.0. The hashing closure is digest `=0.11.3`
(`default`, `block-api`), block-buffer `=0.12.1`, crypto-common `=0.2.2`,
hybrid-array `=0.4.15` and typenum `=1.20.1` (`const-generics`), also MIT OR
Apache-2.0. No third-party type appears in the OAuth primitive interface.

## Approved native terminal libraries

On every non-browser target, `maestro-tui-crossterm` uses Tokio `=1.53.2` (defaults off; `rt`,
`time`) for its caller-driven local task set, `chrono =0.4.45` (defaults off; `clock`) for log
file names and the runtime host's UTC time, and `rand =0.10.3` (defaults) for the host's log
nonce. On Unix it adds rustix `=1.1.5` (defaults off; `std`, `event`, `fs`, `stdio`,
`termios`) on the actual standard descriptors and Tokio's `net`, `signal` and `macros`.
Rustix is Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT, Tokio is MIT and chrono and rand
are MIT OR Apache-2.0. The crate does not use crossterm on Unix, because crossterm prefers the
controlling terminal over the redirected descriptors. Its tests add Tokio `test-util` and
`sync` on every non-browser target, plus rustix `pipe`, `pty` and `process` on Unix. No library type appears in a public
interface except Tokio's `LocalSet`, which the caller supplies to `ProcessTerminal::new`.

## Approved theme libraries

`maestro-theme` uses `jsonschema =0.58.5` (MIT; defaults off, offline) for runtime
theme admission, `indexmap =2.14.2` (default `std`) for ordered color records, and
the workspace `serde` (`derive`, decoding the brand pack), `serde_json`, `num-traits`. Outside browser targets it also uses
`icu_collator =2.3.1` and `icu_locale_core =2.3.0` (defaults off, `alloc`; Unicode-3.0)
for locale-sorted theme inventories, selecting the locale as the completion
collation does. No third-party type appears in its interface. Its production
dependency closure adds the internal `maestro-request` and `maestro-path` crates.

## Approved filesystem-watch libraries

Outside browser targets, `maestro-watch` uses `notify =8.2.0` with defaults (CC0-1.0) for
non-recursive directory notifications and Tokio `=1.53.2`, defaults off, with `rt`, `time`
and `sync` for the caller-driven local task set, timer and notification channel.
The added closure is `notify-types 2.1.0`, `inotify 0.11.5` and `inotify-sys 0.1.8` (ISC),
`mio 1.2.4`, `bitflags 2.13.2`, `log 0.4.34`, `pin-project-lite 0.2.17`, `same-file 1.0.6`
and `walkdir 2.5.0` (MIT OR Apache-2.0 or MIT), plus `fsevent-sys 4.1.0`, `kqueue 1.2.1` and
`kqueue-sys 1.1.2` on the platforms that use them. No library type appears in a public
interface except Tokio's `LocalSet`, which the caller supplies to
`NativeWatchOperations::new`. Browser callers supply their own `WatchOperations`.
Native watch tests add Tokio `test-util`; theme retains native-only dev Tokio
`rt`, `time` and `sync` for its integration tests.

## Approved subscription listener libraries

`maestro-models` uses native-only Hyper `=1.11.1` (defaults off, `server`,
`http1`), hyper-util `=0.1.21` (defaults off, `tokio`) and http-body-util
`=0.1.5` (defaults off). Native Tokio `=1.53.2` adds `net` to its existing
`rt-multi-thread`, `sync` and `time` features. All four libraries are MIT;
no server dependency is declared for the browser target and no third-party
type appears in the authorization interface.

## Approved response-session socket libraries

Outside browser targets, `maestro-models` uses `tokio-tungstenite =0.30.0` (MIT; defaults
off, `connect` and `rustls-tls-native-roots`) for the authenticated socket upgrade, framing and
TLS with the platform's trust roots. Its closure adds `tungstenite 0.30.0` (MIT OR Apache-2.0),
`sha1 0.11.0` (MIT OR Apache-2.0) and `const-oid 0.10.2` (Apache-2.0 OR MIT); `digest 0.11.3`
gains `alloc` and `oid`. The existing rustls, tokio-rustls, native-certs and aws-lc-rs packages
are reused, and no browser socket library is declared. No library type appears in an interface.

## Approved completion collation libraries

Outside browser targets, `maestro-tui` uses `icu_collator =2.3.1` with default
`compiled_data` and `icu_locale_core =2.3.0` with defaults off and `alloc` for
native label comparison. Both use Unicode-3.0. Their resolved closure includes
ICU collections, locale fallback, normalization, properties, provider and compiled
data (Unicode-3.0), plus `utf16_iter =1.0.5` and `write16 =1.0.0`
(Apache-2.0 OR MIT). Existing supporting packages retain their approved licenses
and the `unicode-ident =1.0.24` hold. No third-party type enters the completion API.
Authored path operations use the shared `maestro-path` utility. Browser callers
supply their own `AutocompleteOperations`, without native filesystem or ICU edges.

## Approved attachment process libraries

Outside browser targets, `maestro-tui` uses Tokio `=1.53.2`, defaults off, with
`process`, `io-util`, `rt` and `macros` for caller-selected attachment search.
Native tests add `net` for child readiness sockets. Tokio is MIT; no runtime type
enters the completion interface. Native consumer tests use fd `10.5.0`, pinned
with the development tools in `mise.toml`.

## Approved credentials configuration libraries

`maestro-credentials` uses `indexmap =2.14.2` with defaults for ordered headers,
plus the existing `maestro-path` utility for authored documentation paths. Native
only Tokio `=1.53.2`, defaults off, enables `rt`, `time`, `process`, `io-util` and
`macros` for helper execution. Indexmap is MIT OR Apache-2.0; Tokio is MIT.
Browser callers supply configuration operations without a native process edge.
Stored credentials add `serde =1.0.229` (`derive`) and the workspace `serde_json`
for records, plus the `maestro-models` edge for environment keys and OAuth records.

## Approved package-source libraries

`maestro-packages` uses `git-url-parse =0.6.0` with defaults (`url`, MIT),
`url =2.5.8` and `percent-encoding =2.3.2` with defaults (MIT OR Apache-2.0),
plus inherited workspace serde_json for raw scoped settings. The selected parser
adds `getset =0.1.7` and `nom =8.0.0` (MIT); it reuses the existing URL/IDNA
closure and the `unicode-ident =1.0.24` hold. Outside browser targets it also uses
Tokio `=1.53.2` (MIT; defaults off, `rt` and `process`) to wait for package
commands; no Tokio type appears in a public interface. Serde derive is test-only
here, as is Tokio's runtime builder. Internal dependencies are settings, resources
and the shared path utility. Native effects use the standard library and Tokio and
remain outside the browser target; browser callers supply `PackageOperations`.

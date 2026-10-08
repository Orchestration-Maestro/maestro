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

## Code quality limits

`just check` enforces at most 5 parameters (at most 1 boolean), 60 lines per
function, cognitive complexity 15, nesting depth 4 and
500 code lines per file; doc comments are not counted. Test directories,
`tests.rs` and trailing test modules do not count against file length. Pedantic
Clippy lints are errors; production code may not use `unwrap`, `expect` or
`panic!`. Unsafe code is forbidden. Thresholds live in
`clippy.toml`; levels live in `[workspace.lints]` and crates inherit them with
`[lints] workspace = true`. Quality-lint allowances are forbidden.

## Documentation

Every change has a clear conventional commit message (the squash commit)
describing user-visible behavior. Pull requests never edit `CHANGELOG.md`.
The changelog is generated from conventional commits at release time.

Every crate root needs `//!` docs and every item, private included, needs
`///` docs, enforced by Clippy.
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
- When present, the composition-root crate owns the binary and only wires.

Conventions enforce the [foundation graph](docs/specs/maestro-port.md#crates-and-delivery-order)
through declared and host-resolved Cargo metadata, including optional, target and
build edges. Classes are distinct from delivery layers: only the binary root,
conventions checker, terminal scenario harness and repository tooling are dedicated.
Repository tooling (`maestro-tooling`) is development-only, never shipped and has
no internal dependencies. Sparse workspaces need no placeholder crates. Frontends
require their full direct sets; the terminal adapter and scenario harness require
toolkit only. Other non-leaves permit subsets.
Production and test graphs are separately acyclic; the internal dev-target
allowlist is empty. The scenario harness is not a general dev-dependency target.

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

Keep one current format for everything. No compatibility code.

## Public text

Describe Maestro in its own words. Name no other agent tool.
Follow the [identity guide](docs/identity.md) for voice, code naming and brand
assets. Consumers read brand values from the pack, never hard-code them.

## Agent skills

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
logic, never edit generated model records directly.

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

`maestro-tooling` alone may use `cargo_metadata =0.23.1` with default features
(MIT). Its dependency closure uses MIT OR Apache-2.0 (camino, cargo-platform,
semver, serde, serde_core, serde_derive, serde_json, itoa, proc-macro2, quote,
syn, thiserror and thiserror-impl), Unlicense OR MIT (memchr),
MIT (zmij), and (MIT OR Apache-2.0) AND Unicode-3.0 (unicode-ident).

## Approved scripted-model libraries

The models crate uses rand `=0.10.3` with defaults. Native scheduling uses
Tokio `=1.53.2`, defaults off, with `rt-multi-thread`, `sync` and `time`.
Browser scheduling uses js-sys `=0.3.106`, wasm-bindgen `=0.2.129` and
wasm-bindgen-futures `=0.4.79` with defaults; getrandom `=0.4.3` enables
`wasm_js`. Compact serde_json `=1.0.151` retains `preserve_order`. These
MIT/Apache-2.0 libraries add no internal crate edge or public runtime API.

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

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
`[lints] workspace = true`. Clippy also rejects the ambiguous JSON map `remove`; see
[quality checks](docs/architecture.md#quality-checks). Quality-lint allowances are forbidden, with one exception.
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
  terminal scenario harness, cancellation leaf, watch leaf, lock leaf, process leaf,
  file-pattern crate, text leaf, JSON leaf, entry and tool record crates and
  brand crate may not. Shared path helpers take supplied directory observations
  and add no global environment, filesystem service, directory cache, scheduler
  or workspace dependency.
- `maestro-text` and `maestro-json` are declared core leaves below layer 0 with
  no workspace dependencies. Text owns common text operations (whitespace,
  lexical comparison, markup escaping, locale construction, newline
  normalization shared with `maestro-tools`); JSON owns JSON
  boundary semantics (object acceptance, canonical keys, truth, compact and
  pretty text). `maestro-entry` and `maestro-tool` define session and tool
  interchange records and depend only on `maestro-request`; `maestro-brand`
  projects a brand pack and depends on text and JSON. Text exists; the other
  four are policy rows until later extraction issues create them; add no `common`, `utils` or
  `shared-types` crate. `maestro-pattern` depends only on text.
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
- `maestro-process` is a declared core leaf below layer 0 with no workspace
  dependencies, including the path and cancellation leaves. Only tools, packages
  and the toolkit may depend on it, on native targets. It owns native child-process
  lifecycles; the callers keep cancellation, termination and settlement policy and
  all output handling. Details: [the spec](docs/specs/maestro-port.md#crates-and-delivery-order).
- `maestro-cancellation` owns cooperative cancellation below layer 0, with no
  internal dependencies. Only models and the toolkit may depend directly on it.
  It uses Tokio `=1.53.2`, defaults off, with `sync` on all targets.
- `maestro-request` defines shared message and model-request records below layer 0:
  model, message, content, usage, stream and diagnostic data, plus `SourceInfo`
  and `Skill`.
  Shared application messages include bash execution, custom, branch summary and
  compaction summary records with their existing `Presence<T>` wrapper.
  It also owns the resolved request-auth record, default zero usage and the
  shared optional-field helper. Only the crates that list it in the spec table
  (models, resources, theme, catalog, session, entry, tool and guest authoring)
  may depend on it; it has no internal dependencies. Keep existing models/resources public paths as re-exports.
  Provider/runtime operations and resource discovery stay in their owners.
  The spec's shared-record amendment governs extraction; update the enforced
  table, inventory, graph policy and architecture tests together with the code.
- Names follow `maestro-<noun>[-<role>]`.
- When present, the composition-root crate owns the binary and only wires.

The guest authoring crate may depend on the request, entry, tool and JSON
owners and never imports native session, tool or catalog runtimes. The terminal
adapter requires the toolkit and `maestro-text` as direct dependencies; `maestro-text` is
created, while the other declared leaf owners stay policy rows until their extraction issues.

Conventions enforce the [foundation graph](docs/specs/maestro-port.md#crates-and-delivery-order)
through declared and host-resolved Cargo metadata, including optional, target and
build edges. Classes are distinct from delivery layers: only the binary root,
conventions checker, terminal scenario harness and repository tooling are dedicated.
Repository tooling (`maestro-tooling`) is development-only, never shipped and has
no internal dependencies apart from the optional utility. Sparse workspaces need
no placeholder crates. Frontends require their full direct sets; the terminal
adapter requires toolkit and text, and the scenario harness requires toolkit only. The utility edge never counts
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
Native targets of `maestro-models` also use `aws-sdk-bedrockruntime =1.148.0` and
`aws-config =1.12.0` for Bedrock credentials, signing, retries and event framing, with
`aws-smithy-runtime-api =1.19.0` and `aws-smithy-types =1.8.1` for the transport bridge, and
`aws-credential-types =1.3.0` for the typed credential errors of the selected-profile sources
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

Tests wait for completion signals; shared CI bounds the job, not each test.
Conventions reject recognized test-owned timer calls (Tokio `timeout`/`timeout_at`,
standard timed receives, condition-variable waits and thread parking), including
file-level import aliases. Production deadlines remain allowed. The scan does not
model local shadowing, expand macros, infer receiver types, resolve arbitrary
re-exports/function values or evaluate compound configurations; those forms and
sleep-based watchdogs require source review.

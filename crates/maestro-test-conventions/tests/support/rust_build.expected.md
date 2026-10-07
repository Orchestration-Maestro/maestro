# Rust build tooling

Rust is managed by rustup using `rust-toolchain.toml`. Repository command tools
are pinned by mise. `just check` runs formatting, Clippy with warnings denied,
strict public documentation and workspace conventions. `just test` runs the
workspace tests; `just ci` runs check before test. Each gate runs its selected
suite once, including every recipe/hook assertion. Caller argument boundaries
remain intact, and `just test` never widens or repeats selected execution.

## Private command boundary

The `repository_tools` executable belongs to `maestro-test-conventions` and
exports no library API. Its std-only modules own child environments, temporary
roots, native Cargo artifact classification, documentation execution,
source-development launching and staged-path capture. The bootstrap scripts
compile it directly with the committed rustup toolchain, without invoking Cargo.
Parallel compilers use distinct temporary directories and publish complete
executables, never shared intermediate object files.

Explicit isolated command execution is available as:

```sh
scripts/run-isolated.sh isolate <executable> [arguments...]
```

PowerShell uses `scripts/run-isolated.ps1` with the same argument boundaries.
The source-development launcher is separate:

```sh
scripts/run-source.sh [--no-env] [arguments...]
```

PowerShell uses `scripts/run-source.ps1`. Both launchers build the checkout with
resolved Cargo, then invoke the built executable from the caller's directory.
Empty strings, Unicode, whitespace, quotes and wildcards remain literal. Both
source launch and developer watch resolve Cargo's artifact directory through
`cargo metadata`, honoring `build.target-dir` and `CARGO_TARGET_DIR`.

## Child environments

Isolated children receive an allowlisted environment, not a credential
blacklist. Fresh, distinct directories provide HOME, temporary storage and all
XDG roots. Unix scratch allocation uses fresh unique directories directly under
`/tmp`, so ordinary nested entrypoints do not accumulate temporary path prefixes
or exceed native Unix socket path capacity. A timed case's nested entrypoints
instead allocate beneath its owned scratch tree, so cancelling that case removes
all of its temporary roots. TMP and TEMP equal TMPDIR. `MAESTRO_NO_LOCAL_LLM=1` disables local
provider discovery. No original authentication file is read, renamed or restored.
Only toolchain/cache locations and explicit build/test controls survive.
Cargo's native test runner also preserves its runtime library search paths and
manifest directory metadata. Documentation execution preserves RUSTDOCFLAGS.

The developer `--no-env` switch removes only its observed provider credential
set. It does not replace HOME or disable local discovery. Bash removes HF_TOKEN;
PowerShell retains it. Both retain KIMI_API_KEY, FIREWORKS_API_KEY and unknown
API-key names. Neither adapter strips any other application argument. The source
launcher computes this environment once and applies it to every child, including
Cargo resolution, metadata, build and the application.

## Native tests and documentation

The native Cargo command dispatcher distinguishes ordinary programs from test
artifacts using Cargo metadata and the `deps` artifact layout. Ordinary programs
retain their environment and are never probed with `--list`. Unknown test
artifacts fail before execution. Models, agent and application harnesses retain
libtest discovery, then run selected exact cases in isolated children with a
30,000 ms deadline per case. Other owners retain the whole libtest harness with
no new deadline. Concurrency comes from `--test-threads`, RUST_TEST_THREADS or
available parallelism. Timeout failures do not prevent other cases reporting.
Each invocation uses the same isolated owner subprocess, with one child in a
new process group. A deadline or runner interruption sends TERM to that owner,
which kills the group immediately, reaps descendants and removes its scratch.
Every Linux owner and runner is a child subreaper: cancellation sweeps all of
its remaining children, killing and reaping until none remain. The sweep is
bounded by the process table, not a timer. Exclusively created short scratch
names keep the six-owner route within native Unix socket path limits. A killed nested owner's descendants
are adopted by the nearest living owner. Each runner also sweeps at the end of
its whole run; a case deadline never sweeps another active case's tree. The
runner trusts its owner to exit after the sweep, with no fallback timer.
macOS kills process groups and waits for its own children but does not adopt
escaped descendants; this is weaker than Linux. Windows uses a kill-on-close
Job Object per invocation, assigning its suspended child before resuming it.
Native macOS and Windows runtime checks remain staged until release validation.
An interrupted timed runner exits 143 for TERM, never success.

Discovery uses the same owner and 30,000 ms deadline as a case. Its own listing
and presentation flags precede caller arguments. Caller presentation flags are
removed only from discovery, before the first literal `--`; case presentation
and filter syntax are preserved. Discovery never executes selected tests.
Per-case execution uses a distinct private libtest outcome log for each child;
executed, failed and ignored counts do not depend on stdout or presentation
mode. Caller-supplied `--logfile` is rejected rather than overwritten by several
children.

`scripts/run-rustdoc.sh` resolves the real rustdoc executable through rustup and
isolates it without recursion. A PowerShell adapter is available for explicit
invocation. Direct `cargo test --doc` isolation on Windows is not yet routed;
this gap is tracked in [issue #186](https://github.com/Orchestration-Maestro/maestro/issues/186).
Linux runtime proofs are required now; native macOS and Windows checks must be
restored before the first release.

The checked-in Cargo configuration routes direct native unit, integration and
conventions tests through the dispatcher. The just check/test/ci and pre-commit
routes also isolate their command children. Direct Unix doctests and public
documentation use the real rustdoc wrapper. Raw test executables are not a
supported isolated route and this development guard is not a security boundary.

## Shared recipe-tool prerequisite

The caller declares `test_tools = true` in `.github/ci.toml`. Shared CI installs
the caller's mise-pinned developer tools before its unchanged workspace test
command, so recipe/hook assertions run by default without ignored cases or
local replay invocations. Tests do not download or install tools themselves.

## Workstation fixture safeguards

Workstation Cargo fixture commands use the installed capped launcher. Nested
launchers receive a DBUS_SESSION_BUS_ADDRESS pointing to the current UID's
systemd user bus, only on that launcher process. Its isolated Cargo child does
not receive the variable, and controlled native/doctest fixtures assert this.
Shared CI without capped calls Cargo directly and never reads this variable.
The isolation allowlist and production runner have no systemd-bus exception.

## Failure evidence

Bash source launch checks executable access; PowerShell retains its file check.
Source launch reports a missing tool as `cargo not found at <resolved-path>. Run
just setup from the repo root first.` Build failures prevent application launch,
and application exit statuses propagate. Isolated spawn failures report
`repository tools: spawn child: <operating-system error>`. Native unknown
artifacts report `repository tools: unknown test artifact: <path>`. A case
deadline reports `repository tools: test timed out: <name>`. Diagnostics never
include credential values. Unix interruption removes only owned scratch and
propagates termination to nested owners, including captured children. Each
owner reaps its child before removing its scratch; an uncatchable kill cannot
guarantee cleanup.

## Pending activation

`just clean` removes Cargo's declared artifact directory; `just build` builds
all existing members. `just prepublish` runs clean, build, then check without
publishing. `just dev` stays running with an event-driven native watcher,
performing an initial selected build and rebuilding after Rust/config changes.
The root selects delivered models and agent packages; `just dev-tsc` selects
models only. `just dev-package <package>` watches one package, with dependencies
built by Cargo. The watcher accepts repeatable `--package <name>` selections;
without one it builds the workspace. Browser watch/compiler/CSS/example
selection activates with the browser crate, not a placeholder build here.
Changes arriving during a build schedule another build; generated target and
Git files do not trigger builds. Manifest, Cargo configuration and workspace
layout changes refresh dependency metadata, external watch registrations and
Cargo target-directory exclusions before the next build. Metadata remains
locked: a stale lock reports an error without exiting or changing the last good
registrations; updating the lock lets the watcher retry. Compiler failures are reported without stopping
the watcher. Launch it through the recipe so interruption terminates its owned
process group. The notify dependency belongs only to this private developer
binary, not to the std-only isolation bootstrap.

Private npm author metadata records the models, agent, application and terminal
package purposes without JavaScript exports or install-time compilation. The
controlled asset copier preserves the library and standalone inventories,
including the standalone omission of export CSS/JavaScript. Extension globs
exclude leading-dot and nested files; named copies are literal, and recursive
docs/examples copies retain hidden and nested files. Missing declared inputs fail. Actual product assets activate with the application owner, native
release staging with #111 and dependency/license qualification with #110.

Inactive recipes fail as `repository tools: inactive hook: <hook>; owner: <owner>`:
browser-smoke (#113 plus browser/application owners), generate-models (#135),
component-build (component author), examples-check (application), profile-tui
and profile-rpc (#167), binary (#111), assets and binary-assets (application).
No absent product crate or artifact is represented as a successful build.
The pre-commit browser-smoke hook requires activation plus a pre-format staged
change under `crates/maestro-models/` or `crates/maestro-web/`, or a root
`Cargo.toml` or `Cargo.lock` change; unrelated staged paths do not trigger it.
The caller-owned `.github/ci.toml` retains all landed wasm owner entries and four
foreign targets. `browser_build` stays false until its real recipe exists; the
shared workflow owns schema validation and guest/native build checks.

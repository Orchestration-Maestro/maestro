# Development

Repository rules are in [AGENTS.md](../AGENTS.md). Commands use the pinned Rust
toolchain and the development tools in `mise.toml` and `mise.lock`.

## Setup and build

The pinned toolchain includes `llvm-tools-preview` for the tooling tests' profile
counter checks. When using another toolchain, install it with
`rustup component add llvm-tools-preview --toolchain <toolchain>` before testing.

```sh
git clone https://github.com/Orchestration-Maestro/maestro.git
cd maestro
mise install
mise exec -- just setup
mise exec -- just build
```

Tool versions stay caller-owned in `mise.toml`. Generated downloads and checksums
in `mise.lock` must cover Linux x64 and ARM64, macOS Intel and ARM64, and Windows
x64 hosts, independently of the compilation target. A successful `mise lock` can
still skip unresolved hosts; check its platform inventory. Follow the
[repository setup guidance](../AGENTS.md#checks) when installing pinned tools.

The commit hook formats and re-stages the staged files, announces `Selected
crates: ...`, then verifies the staged crates and every workspace crate that
depends on them. A staged workspace-wide input runs the full workspace check
instead: a manifest, the lockfile, `.cargo/`, the toolchain, the justfile, Clippy
or mise settings, the hook configuration, the workspace inventory, CI files, root
documentation or assets, or the conventions crate. `_check` is the private recipe
that receives either `--workspace` or repeated `-p <package>` arguments, so the
hook and `check` share one command list; `check` passes `--workspace`.

`setup` installs the commit, merge-commit and commit-message hooks. `build`
generates the native catalog once, then compiles the delivered workspace through
Cargo's dependency graph. See [catalog generation](models/generation.md). `check`
formats hand-written Rust, verifies feature unification, runs Clippy, builds strict public documentation and verifies workspace
conventions. `ci` runs `check`, then `test`. `prepublish` runs `clean`, `build`
and `check`; it never publishes. `clean` removes Cargo outputs, not source files.

The conventions check requires `workspace.dependencies.serde_json` to name
`serde_json` and explicitly enable `float_roundtrip`. Normal member dependencies,
including aliases, targets and optional entries, must inherit a workspace entry
that enables it. Dev/build declarations are outside member inheritance enforcement
and do not supply the required workspace feature; test feature unification can
otherwise hide missing normal precision. The generated `maestro-dependencies`
member is exempt from member JSON inheritance; the workspace `serde_json`
precision requirement still applies.

### Windows x86-64 MSVC builds with Unicode paths

When building from source, `aws-lc-sys 0.45.0` can fail if the Cargo target
directory contains non-ASCII characters. The installed NASM executable can lose
those characters before opening its output object file. This is a dependency
build limitation, not an application path conversion or a footer-test failure;
it does not affect users running a prebuilt application.

Use a dedicated build shell with NASM absent from that shell's `PATH`, keeping
Rust, the Microsoft C++ tools and the pinned development tools available. In a
PowerShell build shell, enable the dependency's shipped assembly objects:

```powershell
$env:AWS_LC_SYS_PREBUILT_NASM = '1'
```

Setting this variable alone is insufficient while NASM remains discoverable.
Keep your Unicode `CARGO_TARGET_DIR` unchanged. This configuration uses the stock
dependency's shipped assembly objects; it does not disable assembly, change the
crypto provider or require a patched dependency. Make these environment changes
only in the build shell, not in your persistent environment, and close the shell
after use.

[Native Windows qualification](https://github.com/Orchestration-Maestro/maestro/actions/runs/39564697731/job/119104068630)
passed the 14 selected application tests with both ASCII and Unicode target
directories, all 127 extension-author tests in the Unicode directory, and one
release-mode author test. With the build environment above, run the same consumer
commands from the checkout using the pinned toolchain and checked-in lock:

```powershell
$env:CARGO_BUILD_JOBS = '3'
cargo nextest run --locked -p maestro-app --test integration -E 'test(/^footer_metadata::/)' --no-fail-fast
cargo nextest run --locked -p maestro-extensions-wasm --no-fail-fast
cargo nextest run --locked -p maestro-extensions-wasm --release -E 'test(/^tests::maestro_component_contract_keeps_sync_and_async_calls$/)' --no-fail-fast
```

## Platform test qualification

Compile every declared browser crate's test and example targets without running them:

```sh
mapfile -t wasm_packages < <(
  python3 -c 'import tomllib; print("\n".join(tomllib.load(open(".github/ci.toml", "rb"))["wasm_crates"]))'
)
wasm_args=()
for package in "${wasm_packages[@]}"; do wasm_args+=(-p "$package"); done
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo build \
  --locked --target wasm32-unknown-unknown "${wasm_args[@]}" \
  --tests --examples --keep-going --message-format=json
```

Native socket, process and runtime fixtures stay with their native tests.
Portable assertions remain in browser targets, with strictly parsed fixture
records owned by their readers. A test that needs a Tokio runtime, a checked
child process, a disposable directory or a repository file read at run time is
native-only: the browser target has no Tokio runtime, no filesystem and no test
binary run, so these tests never execute there. Such a module carries
`#![cfg(not(target_arch = "wasm32"))]`. Classify each witness by the operation it
reaches, not by its file's imports: ungate it, build the browser target and read
what the compiler names. A module-wide condition may cover only native items,
so where a file mixes both kinds only its native items are conditioned and the
portable witnesses keep compiling; a shared support module whose records both
targets read moves its portable slice into its own module. A corpus
whose readers differ per target keeps one fixture and record set per reader
group, so a portable reader never parses a file whose members only native
readers consume. An isolation guard that only reports whether a child was
started keeps its signature on both targets: the native body starts the child,
and the browser body reports that none is available, so the group runs in one
process with every assertion intact. A fixture that builds a shared pointer
builds the owner's declared platform handle, so identity comparisons hold on
both targets.

Compilation does not replace the browser assertions below.
The theme examples `brand_themes` and `browser_author_inputs` run their author
work on native targets; their browser entrypoints do nothing. Native usage
messages and generated filenames are unchanged.

## Browser integration qualification

Lint the browser library, examples and complete integration target with the
pinned toolchain:

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo clippy \
  -p maestro-web --lib --examples --test integration --profile test \
  --target wasm32-unknown-unknown --locked -- \
  -D warnings -D clippy::missing_docs_in_private_items
```

Run the retained sandbox assertions with supplied browser tools:

```sh
export PLAYWRIGHT_MODULE=/absolute/path/to/playwright
export CHROMIUM_PATH=/absolute/path/to/chrome
# Put wasm-bindgen CLI 0.2.129 on PATH.
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- just browser-test
```

The command installs no browser tools. It requires Node >=20, Playwright 1.64.1,
Chromium 156.0.8078.4 (supplier revision 1248), and wasm-bindgen CLI 0.2.129,
matching the locked Rust binding. The shared workflow owns tool acquisition.
Observed versions, checkout SHA and Cargo's emitted artifact paths are printed.

The registered integration target is compiled with the workspace lock but never
loaded in the browser: its libtest startup would execute native tests. Runtime
execution uses an unregistered temporary cdylib pointing at the unchanged
`tests/integration.rs`, with `--cfg test` applied only to its root crate. Scratch
dependencies come from the current manifests. Packaging first fetches the
checkout's locked dependencies, which may require network access on a cold cache.
Offline scratch lock preparation rejects new external package identities. The
authored manifests and lock stay unchanged.
Build outputs remain in Cargo's resolved target directory.

Bindings use `--target web --split-linked-modules` to retain local snippets,
relative fixture imports and native module identity. The runner serves only the
packaged output on an ephemeral loopback port. It checks the entire selected
inventory before invoking assertions, then awaits each export in a fresh page:
26 execution exports, 19 loading exports and 49 contribution exports. Packaging
also builds the two runtime-module fixtures, each from its own manifest and lock
with its own binding glue, into a copy of the fixture package. Contribution
exports run on a page served under a content security policy without
`unsafe-eval` and receive the page's `url => import(url)` as their module import.
The host bundle's digest must match before and after the run. Logs identify each
`START` and `PASS`, each `FAMILY` total, the unchanged `HOST` digest and
`COMPLETE 94/94` on success. `PHASE` records
identify loaded pages, resolved exports, pointer dispatch and pointer acknowledgments
so an interrupted run retains its last observed browser phase.

After each native sandbox click completes, the driver checks the selected
fixture document for a fresh trusted click whose composed path contains its
`#target`. Missing, untrusted or wrong-target delivery fails with
`native click did not reach mounted target <id>` before waiting for the pointer
message. An isolated frame receives its input through its own browser session:
the driver scrolls `#target` into that document's view immediately, then sends
move, press and release there. A frame that shares its parent's renderer keeps
the locator click, because the pinned browser gives it no separate session. This
keeps the containing page's coordinates out of an isolated frame's input path,
which is where a native click used to land; it is not a hit-test-ready barrier,
and the compositor cause of the original stall remains unknown. The check does
not bound a native click that never completes.

Missing exports, synchronous throws, rejected promises, unexpected page errors,
page crashes and unexpected browser disconnection fail the command. One deliberate
policy-negative export, `inline_programs_preserve_text_policies_and_order`, must
produce exactly one `TypeError` with message `window.complete is not a function`
from `about:srcdoc`. Its ordinary assertions still run; absent, repeated or other
errors fail. Error accounting stays attached until the page closes. Browser,
server and temporary preparation are closed or removed on both outcomes; a
cleanup failure does not skip the other resource or replace an assertion failure.

Run the driver regressions with the same supplied environment. The packaging
regression invokes Cargo, so they use the same launcher:

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- \
  node --test crates/maestro-web/tests/sandbox_loading_support/run.test.cjs
```

The tool report page is a second witness in the same job: `analysis_page.cjs`
serves the committed report fixture and answers the page's two pinned library
URLs from cached copies whose SHA-256 digests it checks (a missing copy is
fetched with retries, and a mismatch fails). Any other request outside the
script's own loopback origin is aborted and fails the run, checked after
navigation and again after the last observation, so the bytes the page executes
are the bytes the cache was verified with. The script then observes what the
page paints: a row click must change the selected summary and the canvas bitmap,
a recorded `</script>` payload must stay inert text, and bar fills, chart label
text, heading glyphs, summary text and row borders must show the brand roles.
Canvas bands and screenshots are sampled as pixels, and a chart's own animation
completion is the render signal, so no test here waits on a clock. The native
report test holds the fixture byte-for-byte, so a template change has to move
both.

Canvas session-header tests and offline tool cards are not executed; the latter
still depend on the canvas renderer. Transcript execution is tracked separately
in [the transcript test issue](https://github.com/Orchestration-Maestro/maestro/issues/801).
Browser execution is separate from native tests and coverage. `.github/ci.toml`
requests the shared Chromium job with
`browser_test_crates = ["maestro-app", "maestro-web"]`.
The job runs when the existing package selection includes a requested package,
when a nonempty request changes, or for merge groups and main pushes; advisory-only
runs skip it. Native package selection and the coverage floor stay unchanged.
See the [shared caller contract](https://github.com/Orchestration-Maestro/maestro-rust-workflows/blob/v0.15.0/docs/caller-checks.md#chromium-browser-tests).
A local pass is not hosted-CI proof; activation requires a successful shared job.

## Feature unification

See [dependency locks](dependency_locks.md) for generated-lock ownership,
resolution commands and the distinction between metadata and delivered artifacts.

`maestro-dependencies` is generated by `cargo-hakari` (pinned in `mise.toml`;
settings in `.config/hakari.toml`) to unify dependency features for the five
configured native target triples, subject to the traversal exclusions below.
Members depend on it only under `cfg(not(target_arch = "wasm32"))`; browser and component
builds never reach it. Never edit the marked section of its manifest by hand.

To preserve [runtime library ownership](architecture.md#bounded-ownership-checks),
`.config/hakari.toml` leaves `wasmtime` and `wasmtime-wasi` (the guest crate's test
host) out of the traversal: the unifier lists none of them, and the test host may
build its own copies of shared dependencies. Keep that exclusion; `verify`
honours it.

The same traversal exclusion also leaves egui out, so the web library's dev-only
`default_fonts` feature cannot be promoted into production through the unifier.
Production egui remains defaults-off; native attachment tests use genuine
fallback fonts. See [browser libraries](libraries/browser-frontend.md).

Regenerate after changing a dependency, its features or `Cargo.lock`:

```sh
mise exec -- cargo hakari generate
mise exec -- cargo hakari manage-deps
```

After adding a member, first add `maestro-dependencies.workspace = true` to its
`[target.'cfg(not(target_arch = "wasm32"))'.dependencies]` table, then run both
commands. `just check` runs `cargo hakari verify`, which checks feature
unification within the configured scope and exclusions. It does not prove the
generated section is current or that every member declares the dependency; confirm those with `cargo hakari generate --diff` and
`cargo hakari manage-deps --dry-run`, which exit nonzero when work remains.

## Extension components

```sh
mise exec -- just extension-author-component
```

Builds the example extension as a `wasm32-wasip2` component and adds that target
with `rustup` when it is missing. See [writing an extension](extensions/guest.md).

## Source invocation

```sh
mise exec -- just --justfile /path/to/maestro/justfile run-source --help
mise exec -- just --justfile /path/to/maestro/justfile run-source --no-env --help
mise exec -- just --justfile /path/to/maestro/justfile run-source-windows --NO-ENV --help
```

The caller's working directory and literal arguments reach the application.
The launcher uses the checkout manifest and pinned Cargo, not a global installed
application. `--no-env` removes provider credentials for that child only; it does
not move authentication files. The Windows form compares the switch without ASCII
case sensitivity. The source executable now delegates startup to the command-line
library and wires the concrete chat, RPC, selector and native component adapters.
The separate `maestro-model-account` binary runs the standalone account command.

## Compile watches

```sh
mise exec -- just dev
mise exec -- just models-dev-compile
```

The pinned external watcher compiles initially and queues a rebuild when input
changes during compilation. Earlier terminal output remains visible. At startup,
it registers each existing top-level workspace entry as a literal watch path,
including sibling dependencies and Cargo configuration, except `.git` and the
current target directory when that directory is a top-level entry. A new
top-level entry created after startup is watched only after a restart.
Each change is compared literally with Cargo's current target directory before
compiling, so nested target directories and later target switches are handled by
that dynamic filter. Restart the watcher to update its registered roots after a
target switch. `native-dev` covers delivered Rust owners. `dev` composes it with
the browser author server through native parallel recipe dependencies;
`dev-compile` combines models with the browser compile-only watch, while
`models-dev-compile` selects models only. Missing owners fail.
Decision tests use literal watcher events to prove that configured target writes
are skipped, including bracketed paths; mixed source events and pathless events
compile. The decision also compiles delayed events under an old target after a
configuration switch, if those events reach it. Real-watcher recipe tests wait
for fresh input-specific markers printed by compiled binaries to prove rebuilds
for source, sibling dependency and Cargo configuration changes. They also observe
a marker from a rebuilt development binary, then a fresh sibling marker after
that replacement. Queued unrelated successful builds cannot satisfy these checks.
They retain both output streams in one ordered transcript. The harness also writes
watch-test child lines, setup command lines and blocking-operation phases to
per-test files in `MAESTRO_TEST_DIAGNOSTICS_DIR`, or
`target/browser-author-diagnostics/` when it is unset, outside disposable fixtures
and isolated home directories. Set the variable to an absolute path; a relative
one resolves against `crates/maestro-tooling`. The harness does not capture
commands whose output a test reads only after they exit. A forced-kill
test checks both directory routes and verifies that child lines survive termination
while still reaching stderr and the watch consumer. There are no exact build counts
or negative observation windows. Their watch processes share a test-owned
process group, stopped on completion or panic. Before starting a watch-test
process group, the harness directs its LLVM profiles into that fixture's target
directory, outside the merged coverage profile set, because cleanup can forcibly
stop its descendants.

Browser author tests keep the checked-in full fixture and its independent lock
unchanged. Pipeline-only tests use disposable copies with the minimal bootstrap
source and a reduced lock generated during setup, before preservation snapshots.
Framework, optimizer, dependency-closure, formatter and identity-consumption checks
keep the full fixture. Provision both root and fixture sources explicitly before
testing; cold-cache qualification vendors locked offline sources, then starts with
empty Cargo home and target. See
[browser author locks and writes](browser-build.md#locks-writes-and-direct-cargo-use)
for the write allowlist, formatter exception and direct-Cargo preparation.

Tooling test fixtures share a child-toolchain command builder that removes the
parent runner's coverage flags, profile output path and coverage-wrapper state
only from the child command, leaving the parent's environment unchanged.
It restores the wrapper that preceded coverage, when present. This keeps nested
native and WebAssembly builds independent of parent instrumentation without
changing measurement of the test runner. Native browser-author behavior tests
run the already-built development executable and restore only the caller's
`LLVM_PROFILE_FILE`, when present, to collect its runtime counters. Compiler
flags and coverage wrappers remain removed; real recipe tests and forcibly
terminated watch groups do not use this exception. See the
[browser author guide](browser-build.md) for the fixture-only build contract.
Profile-writer fixtures still request their own instrumentation explicitly.
Fresh tooling and conventions fixture roots carry the checkout's complete `rust-toolchain.toml` declaration before their first
compiler child. Compiler, Cargo and recipe children run beneath that declaration;
the shared command builder sets `RUSTUP_TOOLCHAIN` to the resolved toolchain
recorded when the test was compiled, overriding ambient selectors for every
descendant regardless of working directory. Environment-clearing fixture drivers
reapply that value after clearing. When compilation supplies no resolved toolchain,
the builder leaves normal Rustup selection unchanged.
Clearing a child's environment does not remove the fixture's declaration. Required
components and targets are provisioned before tests, never installed by fixtures.

Native compiler watches do not run catalog generation or copy assets. The
[browser author graph](browser-build.md) also watches its stylesheet and shared
identity inputs through Trunk, without a second watcher implementation.
`models-build` invokes catalog generation once before its explicit Cargo build; ordinary
Cargo compilation, tests and watches remain generation-free.

## Identity and execution modes

### Compiled identity and brand customization

[`maestro_app::configuration::config`](../crates/maestro-app/src/configuration/config/mod.rs)
defines the package and command name `maestro`, display title `Maestro` and
compiled Cargo package version. It re-exports the resource owner's `.maestro`
configuration-directory name and derives `MAESTRO_CODING_AGENT_DIR` and
`MAESTRO_CODING_AGENT_SESSION_DIR` from the command name. The default agent root
is `<home>/.maestro/agent`; a nonempty `MAESTRO_CODING_AGENT_DIR` overrides it.
See [application configuration](application/configuration.md#identity-and-roots)
for root selection. Renaming the executable alone does not change these values.
Changing compiled identity requires source edits and a rebuild; there is no
runtime rebranding configuration API.

Brand customization uses the [brand pack](identity.md#pack-shape-and-swapping),
not installation identity. The pack supplies visual values such as colors,
fonts, glyphs and mark paths. `maestro-brand` parses and projects those values;
`maestro-theme` loads native packs and templates and generates terminal themes.
Changing a pack does not rename the command, environment variables or settings
directory, and does not install that pack into a running application.

Use the existing [brand theme example](theme.md#brand-pack) to try a pack without
changing shipped files. Run this from the checkout root:

```sh
scratch=$(mktemp -d)
mise exec -- cargo run --locked -p maestro-theme --example brand_themes -- \
  assets/brand/packs/signal.json "$scratch"
test -s "$scratch/dark.json" && test -s "$scratch/light.json"
```

The output directory holds generated themes for inspection; remove it when
finished. Runtime preferences belong to [settings](settings.md), where `theme`
selects a terminal theme. They do not replace the compiled identity or select a
brand pack. See [brand projection](brand.md) and
[native pack loading](theme.md#brand-pack) for the existing author interfaces.

### Central asset paths

Use `maestro_app::configuration::config` rather than paths relative to the
current Rust source file. Its native `get_package_dir`, `get_readme_path`,
`get_docs_path`, `get_examples_path` and `get_theme_directories` query the running
installation. `config::paths` supplies explicit-input operations for themes,
interactive assets and export templates; callers supply the package directory
and layout. These queries return paths, not a guarantee that assets exist.

A build whose compile-time crate directory remains below its source workspace
uses that workspace root and package layout. Otherwise it uses standalone layout
with assets beside the executable. A nonempty `MAESTRO_PACKAGE_DIR` overrides
package-root discovery. See
[installation paths](application/configuration.md#installation-paths)
for layout selection and individual asset routes. Installed distributions still
need prepared assets; the copy recipes below do not create missing inputs.

The copy recipes default to `maestro-app-assets` and `maestro-binary-assets`
under Cargo's configured target directory. `app-clean` removes both asset trees;
workspace `clean` removes the complete configured output. Explicit copy
destinations remain available.

`copy-assets SOURCE DESTINATION` copies prepared library themes, images, export
templates and vendor assets. `copy-binary-assets SOURCE DESTINATION VIEWER PACKAGE`
resolves the `maestro` manifest through Cargo metadata and copies metadata, README,
release-generated history, themes, images, the supplied viewer, docs and examples.
Themes and images come from `SOURCE`, the `export-html` directory from `VIEWER`,
and the README, history, documentation and examples from the package directory
`PACKAGE`, which defaults to the justfile directory. An empty `VIEWER` directory
copies as an empty `export-html` directory into a fresh destination. In an existing
destination, the `export-html` directory keeps the entries it already holds and
gains nothing from the viewer.
These commands do not generate a viewer or regenerate assets. A missing or unusable
input fails with a message naming the path it could not read; already copied groups
and unrelated destination files remain. `app-build` compiles
before library copying. `build-binary` compiles toolkit, models, agent, application
and executable in that order before standalone copying. Use the
[browser author build](browser-build.md) for browser bundles;
[session serialization](export.md) accepts a supplied viewer bundle rather than
building it.

## Source owners

Start at the owner of the behavior, not the executable's wiring. The
[crate graph](specs/maestro-port.md#crates-and-delivery-order) defines allowed
dependencies; this map points to the implementations and their usage guides.

| Work | Source owner | Guide |
| --- | --- | --- |
| Native startup wiring | [`maestro/src`](../crates/maestro/src/main.rs); `maestro-cli` owns argument preparation, startup selection and print output | [Command line](cli.md) |
| Session coordination and embedding | [`maestro-app/src`](../crates/maestro-app/src/lib.rs) | [SDK](application/sdk.md) |
| Model invocation, protocol conversion and streams | [`maestro-models/src`](../crates/maestro-models/src/lib.rs) | [Model records](records.md) |
| Conversation loop, tool batches and steering | [`maestro-agent/src`](../crates/maestro-agent/src/lib.rs) | [Agent](agent.md) |
| Terminal components, input and frame rendering | [`maestro-tui/src`](../crates/maestro-tui/src/lib.rs) | [Components](terminal/components.md) |
| Interactive chat presentation | [`maestro-chat/src`](../crates/maestro-chat/src/lib.rs) | [Chat](chat/interactive-mode.md) |
| Native terminal effects | [`maestro-tui-crossterm/src`](../crates/maestro-tui-crossterm/src/lib.rs) | [Native terminal](terminal/native-terminal.md) |
| Standard-stream RPC adapter | [`maestro-rpc/src`](../crates/maestro-rpc/src/lib.rs); the connection runner belongs to `maestro-app::rpc_mode` | [Daemon protocol](daemon-protocol.md) |
| Extension author API and canonical component interfaces | [`maestro-extensions-wasm/src`](../crates/maestro-extensions-wasm/src/lib.rs) and its `wit/` directory; native execution is adapted by `maestro-extensions-wasmtime` | [Guest authoring](extensions/guest.md) |

## Debugging

In the interactive mode, Shift+Ctrl+D writes a debug capture to
`~/.maestro/agent/maestro-debug.log`, or to `maestro-debug.log` in the directory
`MAESTRO_CODING_AGENT_DIR` names. The capture holds the terminal size, every
rendered line with its visible width and escaped ANSI sequences, and the session's
messages as JSON lines. Submitting `/debug` writes the same capture. See
[debug capture](chat/commands.md#debug-capture). The interactive owner supplies this
behavior; development commands do not create a replacement logger.

## Tests

```sh
mise exec -- just test
mise exec -- just test-offline
mise exec -- just test -p maestro-tooling maestro_source_without_switch_preserves_environment
```

The broad test recipes run cargo-nextest, then Cargo doctests only after success.
They forward literal arguments to both phases; use options accepted by both,
such as name filters and `-- --ignored`, `-- --exact NAME` or `-- --nocapture`.
An unmatched ordinary-test filter fails rather than passing an empty selection.
Use direct `cargo test --doc` for doctest-only runs and direct `cargo nextest run`
for runner-only flags or target selection. `tui-test-ansi` runs only nextest.

### Test time budget

Nextest fails a test at 30 seconds by default. Fix slow tests at their cause;
do not raise the default or add headroom to an exception. A temporary exception
in `.config/nextest.toml` must select one test or one shared fixture group, and
name the measured work and an open issue that will remove it:

```toml
[[profile.default.overrides]]
filter = "package(=maestro-example) & test(=component::loads)"
# Measured: 43.549s; cause: compiling the component fixture; #3589
slow-timeout = { period = "44s", terminate-after = 1 }
```

Use the measured duration, rounded up to whole seconds, as the temporary limit.
Write `period` as positive whole seconds (`"Ns"`) and set a positive
`terminate-after`; their product is the test's limit. Keep the `Measured:`
seconds, `cause:` description and `#NNNN` issue in comments immediately above
`slow-timeout`. Each exception needs its own comment; another override's comment
does not apply. A limit of 30 seconds or less needs no exception comment.

The conventions check rejects unbounded limits, defaults over 30 seconds and
undocumented raised overrides in every profile. It checks references offline;
the author and reviewer verify with GitHub that the linked issue is still open.
The change fixing the slowness must delete its override as well as close the
issue. Existing measured exceptions are tracked in #3589.

Development and test profiles use `debug = "line-tables-only"` for workspace crates:
backtraces retain file:line for workspace code, without debugger variable views.
Dependencies build without debug info, so their frames show function names only;
this keeps target directories about a sixth smaller.
They also build `cranelift-codegen` and `regalloc2` optimised, because tests
compile extension components and an unoptimised code generator makes each
compile about five times slower, and `syn` and `proc-macro2`
optimised, because the conventions tests parse every workspace source; release
builds are unchanged.

Before guest-authoring tests, nextest's shell launcher locates the active
artifact directory through its documented platform library search path. It runs
the `prepare_author_component` example that the test build put in that directory,
as long as the example is newer than its source, the shared build and component
discovery sources, the package manifest and `Cargo.lock`. Running it through Cargo
instead would resolve features for that one package and recompile the engine. When
the example is missing or older, as after a `--lib` or filtered selection, the
launcher builds it through Cargo in the same target directory and profile before
running it directly.
The launcher fails if the directory cannot be resolved rather than building
elsewhere. It uses `NEXTEST_LD_LIBRARY_PATH` on Linux,
`NEXTEST_DYLD_FALLBACK_LIBRARY_PATH` on macOS and `PATH` through Git's shell
on Windows, where the shell converts the search list to colon-separated paths.
Before running the example, `normalize-component-cache.sh` gives executable
artifacts in the profile directory, `deps` and `examples` their newest existing
modification time. This advances older outputs without making them older than
their compiler inputs. It skips an executable older than a library output in
`deps`: Cargo rebuilds an executable only while a dependency's output is newer
than it, so advancing a binary that a rebuilt library has outdated would hide the
rebuild. A skipped binary compiles its components itself on first use. The
Git-pinned engine includes that timestamp in its cache namespace, so the preparer
and consuming test binaries share compiled code.
The example builds every exported component, warms its Wasmtime compilation cache
in the default user cache on up to four threads and exports its path to the tests'
processes. Component contents, compiler settings and CPU features still determine
cache compatibility.
The shared component builder resolves these source-marked paths before invoking
Cargo. The component build removes `RUSTC_WRAPPER`, where the coverage tool puts its
instrumenting wrapper, and keeps `CARGO_BUILD_RUSTC_WRAPPER`, where shared CI
sets the compile cache. So the component stays uninstrumented while its
compiles still reach the cache.
Plain Cargo tests retain their self-building fallback. Response-account native
callback tests use assigned listener addresses through the private binding seam;
the advertised redirect URI remains unchanged. Public protocol, account and
provider checks run in checked child processes with only the child's callback
host set to a single space, exercising manual fallback without a listener.
Callback tests that still bind a fixed port belong in an `oauth_callback` module,
which joins a single-thread nextest group. Browser author-build tests also
join a single-thread group because Trunk shares binding/optimizer intermediates;
their file lock keeps plain Cargo tests safe. Other tests remain parallel.

Only `test-offline` sets local-model suppression, removes its explicit
provider variables and temporarily moves `$HOME/.maestro/agent/auth.json` to the
fixed backup path, restoring exactly the entry moved by that run on every
ordinary return. Pre-existing backups are left untouched when no auth entry moves. It runs
`just test` in the invocation directory and ignores wrapper arguments. It is not
a universal sandbox. Do not use it alongside another process editing that auth
file. Tooling regressions instead use disposable authentication fixtures and
controlled processes, without provider calls or operator credentials.

Pull-request CI selects tests from changed crate ownership, declared test inputs
and global fallbacks, then includes dependent crates. Use `test_inputs` in
`.github/ci.toml` to declare additional paths, mapped to the packages whose tests
read them. A file owned by another workspace member already selects that member
and its dependents. Only otherwise unowned, undeclared paths use the all-or-none fallback:
they select every test, or none when the file is Markdown or under `docs/`.

### Child-process tests

A test that re-runs one test in a fresh process starts it through
`maestro-test-process`, a native dev-dependency, instead of building a `Command`
for the test executable. `TestProcess::current(leaf)` resolves the running test's
full name, `TestProcess::named(identity)` selects a sibling by its complete name
(derive it from the function item with `test_name`), and the consuming
`output`, `status`, `spawn`, `status_with_stdout` and `output_with` operations
run it. The owner appends the exact filter and harness flags, confirms the test
is listed and eligible (an ignored test runs only after `.ignored()`), and checks
that the child's standard output begins with the one-test start prologue. A stale
or misspelled name therefore fails instead of passing with zero tests. A checked
outcome proves the test started, not that it passed: keep exit, signal and product
assertions in the test.

Choose the route by stream use. `output` captures both streams and returns them
unchanged. `spawn` reads only the prologue from a piped standard output and replays
it, preserving ready lines and acknowledgements. A test that depends on closing
a live pipe end must first call `run_alone(test)` and return when it returns
`true`: the transaction then runs in a fresh process with no sibling tests that
could fork and inherit its pipe ends. `status` replays captured streams afterwards and is not a
live route. `status_with_stdout` requires an empty regular file with its cursor at
zero, sends standard output there and checks the prefix through that same open
file. Nonempty files or nonzero cursors are rejected before launch, including
append-mode files containing earlier output. `output_with` hands the final
argument list to production code under test and accepts no environment, directory,
descriptor or shell setting. `shell_setup` (Unix) runs shell text before the
executable replaces the shell, in the interpreter the caller names by absolute
path: neither this process's `PATH` nor the child's selects it, and a relative
path is refused before anything runs.

Child-test suites use `TestProcess::current` for the running test or
`TestProcess::named` for a supplied complete identity. `output()` checks that the
selected test starts; callers still check its exit status and scenario output.
`env_clear()` preserves the inherited coverage profile destination, and later
caller edits can override or remove it. Live protocols retain `CheckedChild` and
its replaying `CheckedStdout` until the exchange completes, then check successful
reaping. Settings helpers select supplied complete nested identities with `named`
and apply shell setup through `shell_setup`, keeping reports separate from exit
status checks.

Cases that read process variables go through `run_per_environment(test, cases,
environment)`. It groups the cases by the variables `environment` returns and
reruns the test once per group, in a child that holds exactly those variables
after `env_clear()`, plus the parent's temporary directory unless the case sets
one. In the parent, `run_per_environment` returns `None` once every child
passes; in a child it returns that group's cases in their original order, and the
test checks them. Cases that share variables therefore share one child and one
fixture parse, so don't spawn a child per case yourself. A case that needs a fresh
process for other state, such as a process-wide counter or files created per
case, keeps its own child. Use `TestProcess::case(id)` to select that case. Set a
child's temporary directory with `TestProcess::temp_dir(path)`, which writes the
variables each platform reads, never `TMPDIR` alone. A child that replaces `HOME`
and loads a prepared component passes `user_cache_home()` as `XDG_CACHE_HOME`,
so it reuses the setup's compiled code instead of compiling the component again.
Read child roles through `maestro_test_process::selection()`, not environment
variables. `selection.is_child()` distinguishes the parent from a child.
`selection.runs_case(id)` accepts the parent, wildcard children and the child
selected for that id. `selection.runs_group(test)` accepts the parent, wildcard
children and environment-group children of that test. Case children do not run
groups, and group children do not run individually selected cases.
Use case identifiers for suite roles and nested stages; keep fixture data separate
from selection, and call `env_clear()` before `case(id)` when clearing is needed.

A test that denies access through mode bits calls
`maestro_test_directory::deny(path, mode)` and skips the denial when it returns
`None`: a privileged run, such as root in a container, is not refused. Package
tests that reach the offline flag run in the package suite's `case_in_child`,
which starts every case online and gives it its own temporary directory, so the
flag in the developer's shell never reaches them.

Credential tests acquire `SERIAL` before `PROCESS_CREATION` when both are needed.
Configure the builder first, hold the creation guard only around checked `spawn`,
and release it before waiting for completion. Pipe both output streams explicitly;
`spawn` otherwise retains inherited stdin. Live pipe-closure transactions use the
single-test isolation described above, not a process-wide owner lock.

### Compile cache warm-up

Shared CI restores a compiled-cache snapshot in each Linux job, but only pushes
to `main` save one. `.github/workflows/warm-cache.yml` runs on every push to
`main` and calls the shared workflow with `warm-cache: true`. That builds the
full coverage test graph, then runs only the tests selected by
`warm_test_filter` in `.github/ci.toml`. Today that is one guest-authoring test,
so the author component's compiles enter the snapshot that merge-queue test
shards restore. The snapshot is capped at 1 GiB and evicts its least recently
used entries, so the selection stays narrow. The warm-up is never a required
check, and a newer push cancels a running one. Its `uses:` pin moves with each shared-CI
release re-pin
([Orchestration-Maestro/.github#231](https://github.com/Orchestration-Maestro/.github/issues/231)).
A stale pin fails nothing; it only costs snapshot hits.

## Fuzzing

`fuzz/` holds coverage-guided fuzz targets for the parsers that read untrusted
text. It is a separate Cargo workspace, so `just check`, `just test` and shared
CI never build it. Each target calls a public entry point and reports a crash,
a hang or runaway memory as a finding.

| Target | Entry point |
| --- | --- |
| `markdown` | `maestro_markdown::parse_with_options` with every caller grammar, then the terminal `Markdown` component rendering an assistant reply at width 80 |
| `frontmatter` | `maestro_resources::parse_frontmatter`, which the skill and prompt loaders call |
| `terminal-escapes` | `maestro_terminal::Terminal::write` for program output and `maestro_tui::StdinBuffer::process` for keyboard input, with the same bytes |
| `protocol-frames` | `maestro_protocol::frames::read_message`, decoding consecutive client and server frames |
| `tool-call-arguments` | `maestro_models::parsed_arguments` |

The text targets skip input that is not UTF-8, because their callers only pass
decoded text. Seeds come from the existing test fixtures and live in
`fuzz/corpus/<target>/`. The Markdown seeds include inputs that reproduce known
defects: a 60-level quote, a 150-line and a 10,000-line reply-style quote, and
5,000 nested quote markers.

Each target has recorded libFuzzer limits. A run that exceeds one reports a
finding:

| Target | `-max_len` (bytes) | `-timeout` (seconds) | `-rss_limit_mb` |
| --- | --- | --- | --- |
| `markdown` | 131072 | 60 | 2048 |
| `frontmatter` | 4096 | 10 | 2048 |
| `terminal-escapes` | 4096 | 10 | 2048 |
| `protocol-frames` | 4096 | 10 | 2048 |
| `tool-call-arguments` | 4096 | 10 | 2048 |

The Markdown limits keep the largest seed whole. Under the instrumented fuzz
build, 128 KB of ordinary Markdown takes 12 to 23 seconds, depending on
machine load. Each defect seed exceeds the timeout or the stack.

### Run one target locally

You need Linux or macOS on x86-64 or Aarch64, a C++ compiler with C++11
support, a nightly Rust toolchain and cargo-fuzz:

```sh
rustup toolchain install nightly
cargo install cargo-fuzz --locked
```

From the repository root, fuzz the Markdown target for one minute:

```sh
mkdir -p target/fuzz-corpus/markdown
cargo +nightly fuzz run markdown target/fuzz-corpus/markdown fuzz/corpus/markdown -- \
  -max_total_time=60 -max_len=131072 -timeout=60 -rss_limit_mb=2048
```

New inputs go to the first directory, under the ignored `target/`, so the
committed seeds stay unchanged. Use another target's name, corpus and limits
from the table to run it. A finding stops the run and leaves its input in
`fuzz/artifacts/<target>/`. Pass that file in place of the corpus directories,
with the same limits, to reproduce it.

The Markdown seeds stop a run at the first known defect until its fix lands.
To explore new inputs in the meantime, leave the seed directory off the command.

### Nightly findings

The shared nightly fuzz job runs each declared target for ten minutes with
these limits. It never gates a pull request or the merge queue. A crash, hang
or out-of-memory input becomes an issue in this repository, one per distinct
crash signature, with the minimized input attached to the job's run as an
artifact. A repeat of an open finding adds a comment to that issue.

## Package commands and roles

`models-`, `agent-`, `tui-` and `app-` each offer `clean`, `build`, `dev`, `test` and
`prepublish` forms. Package prepublish is clean then build, without root checks.
`tui-test-ansi` runs only the `ansi_wrapping` module in the integration-test target of the
terminal toolkit. These commands fail when their owning crate is absent; no
placeholder product package is installed.

Browser author build, check, clean, watch and preview commands are available; see
[the browser author guide](browser-build.md) for their fixture-only boundary.

The following command groups are unavailable until delivered by their owners:

| Command group | Corresponding command | Owning issue | Availability |
| --- | --- | --- | --- |
| Profiling and diagnostics | `profile:tui`, `profile:rpc` | [#241](https://github.com/Orchestration-Maestro/maestro/issues/241) | unavailable until delivered |
| Version, publish and release | `version:patch/minor/major/set`, `publish`, `publish:dry`, `release:patch/minor/major` | [#81](https://github.com/Orchestration-Maestro/maestro/issues/81) | unavailable until delivered |
| Real platform archives | `build-binaries.sh` | [#111](https://github.com/Orchestration-Maestro/maestro/issues/111) | unavailable until delivered |


- `maestro-models`: model interfaces, providers and catalog data.
- `maestro-agent`: [live conversation state and queued input](agent.md), with
  [awaited and observable conversation operations](loop.md) and the
  [proxy stream client](proxy.md).
- `maestro-tui`: terminal components.
- `maestro-app`: shared application operations.

Currently `maestro-tooling` automates repository development and
`maestro-test-conventions` verifies workspace structure. Neither ships with the
application. Their integration tests import their libraries.
The conventions checker shares its private manifest reader and source traversal
helpers; span exclusion and test-owned timer discovery keep separate traversal
policies.

Select individual toolkit tests through `tui-test` using their module-qualified
names, for example `just tui-test -- ansi_wrapping::name --exact`.

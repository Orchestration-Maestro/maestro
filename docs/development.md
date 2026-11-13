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
just setup
just build
```

Enable mise in your shell (or prefix a command with `mise exec --`) so `just`
and the other pinned tools are on `PATH`. Rust comes from `rust-toolchain.toml`
through rustup, not mise.
`just setup` also fetches the examples tree from maestro-packages into
`examples/`.

`build`, `test`, `check` and `ci` first build the sandbox runtime script, which needs
Emscripten SDK 6.0.10 and the wasm-bindgen CLI 0.2.129 on `PATH`; install them as
described in [Sandbox runtime bundle](#sandbox-runtime-bundle) before the first run.

Tool versions stay caller-owned in `mise.toml`. Generated downloads and checksums
in `mise.lock` must cover Linux x64 and ARM64, macOS Intel and ARM64, and Windows
x64 hosts, independently of the compilation target. A successful `mise lock` can
still skip unresolved hosts; check its platform inventory. Follow the
[repository setup guidance](../AGENTS.md#checks) when installing pinned tools.

The commit hook selects only the crates holding staged files, including member
manifests. It runs `cargo fmt -p <package>` for those crates, re-stages the captured
files that remain on disk, announces `Selected crates: ...`, then runs
`cargo clippy -p <package> --all-targets --locked -- -D warnings`. Multiple crates
use repeated `-p` arguments. Formatting failures and Clippy warnings block the
commit. Files changed by formatting but absent from the captured index stay
unstaged.

A staged `maestro-web` is linted through `just _check -p <package>` instead, because
its build script embeds the generated sandbox runtime script, which that recipe builds
first (see [Sandbox runtime bundle](#sandbox-runtime-bundle)). This adds minutes to
commits that touch the crate.

Root inputs such as `Cargo.lock`, the workspace manifest and documentation select
no crates. They never widen a mixed change's selection. With no staged crate, the
hook skips formatting and Clippy. The full conventions suite runs only for staged
paths under `crates/maestro-test-conventions/`, guarding the checker itself. The hook
runs it as `just test-conventions`, which prepares the native fixture the probes
install and then runs the suite with nextest.
Browser smoke runs when the browser entry is present and a staged crate owns a
browser compile witness, guarding targets outside native Clippy.

For changed crates, run `just _check -p <package>` with repeated `-p` arguments
for multiple crates. `_check` verifies feature unification, runs native Clippy,
Windows-GNU Clippy and the browser-target lint, builds strict public
documentation and checks workspace conventions. The native and Windows-GNU passes
use the same package selection with `--all-targets --locked -- -D warnings`; they
add no reverse dependents. The browser-target lint reads `wasm_crates` from
`.github/ci.toml` and follows the same selection, so a local check and shared CI
lint one crate list. It lints library targets only, because an integration test
may name a crate that exists on native targets alone. A package argument it cannot
match to one declared crate, such as a pattern or a version, fails the check
instead of linting nothing.
CI checks dependents and workspace-wide inputs. `just check` explicitly selects
`--workspace` for every pass on every development host. The commit hook stays
native-only.

The pinned `rust-toolchain.toml` provisions `x86_64-pc-windows-gnu`. To repair a
missing standard library, run `rustup target add x86_64-pc-windows-gnu` from the
checkout. Install MinGW-w64 too: on Ubuntu use
`sudo apt-get install gcc-mingw-w64-x86-64-posix`; on macOS use
`brew install mingw-w64`; on Windows use a MinGW-w64 development shell with its
compiler on `PATH`. Cross-builds need `x86_64-w64-mingw32-gcc` on `PATH`, or Cargo's
C dependencies need `CC_x86_64_pc_windows_gnu` set to that host's MinGW-w64 compiler.
Missing prerequisites fail the check; there is no native-only fallback.
Windows-GNU checks `cfg(windows)` and `cfg(unix)` boundaries, not MSVC-specific
code or ABI behavior. Required native Windows-MSVC CI and native runtime
qualification remain in place.

`setup` installs the commit, merge-commit and commit-message hooks. `build`
generates the native catalog once, then compiles the delivered workspace through
Cargo's dependency graph. See [catalog generation](models/generation.md). `check`
formats hand-written Rust, verifies feature unification, runs native, Windows-GNU
and browser-target Clippy, builds strict public documentation and verifies
workspace conventions. `ci` runs `check`, then `test`. `prepublish` runs `clean`,
`build` and `check`; it never publishes. `clean` removes Cargo outputs, not source
files.

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
just prepare-tests
cargo nextest run --locked -p maestro-app --test integration -E 'test(/^footer_metadata::/)' --no-fail-fast
cargo nextest run --locked -p maestro-extensions-wasm --no-fail-fast
cargo nextest run --locked -p maestro-extensions-wasm --release -E 'test(/^tests::maestro_component_contract_keeps_sync_and_async_calls$/)' --no-fail-fast
```

## Platform test qualification

Native path assertions use the host's separators. Build lexical expectations
with `maestro_path::join`, including paths embedded in diagnostics and Cargo
metadata. Leave authored path inputs and production output unchanged. On
Windows, a rooted path without a drive, such as `/w`, uses the current drive;
a supplied home does not choose the drive for an unexpanded name such as `~x`.
A passing Linux run does not qualify a Windows assertion: run it on native
Windows too. Shared fixtures build an installed location once, in their
support module, so every test that names it uses the same native path.

Compile every declared browser crate's test and example targets without running them:

```sh
mapfile -t wasm_packages < <(
  python3 -c 'import tomllib; print("\n".join(tomllib.load(open(".github/ci.toml", "rb"))["wasm_crates"]))'
)
wasm_args=()
for package in "${wasm_packages[@]}"; do wasm_args+=(-p "$package"); done
cargo build \
  --locked --target wasm32-unknown-unknown "${wasm_args[@]}" \
  --tests --examples --keep-going --message-format=json
```

`just _check` lints the declared browser crates for `wasm32-unknown-unknown` in
library targets only. Lint one crate's test and example targets directly where
they compile for the browser target:

```sh
cargo clippy --locked --target wasm32-unknown-unknown \
  --all-targets -p maestro-models -- -D warnings
```

`clippy --all-targets -D warnings` also rejects unused native-only helpers that
`check --tests` leaves as warnings. A crate whose integration tests name native-only
test crates refuses that target; lint its library target instead.

A browser compile witness, an example target whose body only compiles for the
browser target, is not covered by native Clippy. `.github/ci.toml` declares each
witness under `wasm_examples`, and the shared wasm job compiles every declared one
for `wasm32-unknown-unknown` on each pull request, stopping at the first failure.
`just check-browser-smoke` compiles the same witnesses locally and stops at the
first failure, printing the path of the diagnostics log. The commit hook runs the
same command when a staged crate owns a witness. `maestro-tooling` holds the one
witness list: its browser smoke command compiles it, and a test fails when the
`wasm_examples` declaration and that list differ.

Checking one crate at a time prevents another crate's development features from
hiding missing helper activation. Portable helper imports, owner modules and
`test-support` development dependencies must all be available on browser targets.
Keep native-only collaborators and their imports conditional. Library and example
checks alone do not compile integration tests.

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
The theme generators `brand_themes` and `browser_author_inputs` are repository
scripts under `crates/maestro-theme/scripts`; they run their author work on native
targets, and their browser entrypoints do nothing. Native usage messages and
generated filenames are unchanged.

## Sandbox runtime bundle

The in-frame sandbox runtime is a Rust program in `crates/maestro-web/runtime`, built for
`wasm32-unknown-emscripten` into one classic script. The script is a build output:
`just sandbox-runtime` writes it to `target/sandbox-runtime/maestro-sandbox-runtime.js`,
which Git ignores, and the `maestro-web` build script points the crate at that file. The
package has its own manifest and lock, outside the workspace, so the chat application never
builds for the frame.

Install Emscripten SDK 6.0.10 and the Rust standard library for the target, then put
`emcc` and the wasm-bindgen CLI 0.2.129 on `PATH`:

```sh
rustup target add wasm32-unknown-emscripten
CARGO_BUILD_JOBS=3 just sandbox-runtime
```

Run the step before anything that builds `maestro-web`. `just check`, `just _check`,
`just test`, `just build` and `just native-dev` run it first.
A plain `cargo` build without the file fails in the `maestro-web` build script with one
message that names the step. The command refuses a missing or unpinned compiler (naming the
tool and its release), a library outside the audited list and script text that an inline
script cannot carry. It wraps the script in a function so no glue name reaches the frame's
global scope, and passes the compiler flags in Cargo's encoded form so a checkout path with
spaces builds. It writes the file only when the bytes change, so an unchanged build does
not rebuild `maestro-web`.

The command records the checkout as `/checkout` and each dependency's sources as
`<name>-<version>`. These stable names keep the script the same on every machine: no Cargo
home or checkout path reaches the embedded module. A web crate test fails when the script
names any other absolute source path.

A plain Cargo build for the target links the runtime with the package's own settings.
Its build script passes the Emscripten link settings to the binary; its manifest sets
`panic = "abort"` in the dev and release profiles. The generator command adds only compiler
flags, source path names and the linker map for the audit. The package repeats the
workspace lint table in its manifest. Browser linting checks it for
`wasm32-unknown-emscripten`.

The build box's saved setup carries the SDK. Shared CI installs the same release because
`.github/ci.toml` lists the package in `emscripten_crates`: merge groups build it with a
plain `cargo build`. Every CI job that compiles `maestro-web` must run the step first.

The workspace conventions check fails when Git tracks a `.js`, `.mjs`, `.cjs`, `.jsx`,
`.ts`, `.tsx`, `.py`, `.sh` or `.ps1` file. Generated browser code comes out of the build
under `target/` and is never committed.

## Browser integration qualification

Lint the browser library, examples and complete integration target with the
pinned toolchain:

```sh
cargo clippy \
  -p maestro-web --lib --examples --test integration --profile test \
  --target wasm32-unknown-unknown --locked -- \
  -D warnings -D clippy::missing_docs_in_private_items
```

Run the retained sandbox assertions with supplied browser tools:

```sh
export CHROMIUM_PATH=/absolute/path/to/chrome
# Put wasm-bindgen CLI 0.2.129 on PATH.
just browser-test
```

The command installs no browser tools. It requires Chromium 156.0.8078.4
(supplier revision 1248) and wasm-bindgen CLI 0.2.129, matching the locked Rust
binding. It also builds the sandbox runtime script, but only when the checkout lacks one.
A script already under `target/sandbox-runtime/` is used as it is, the same file the
`maestro-web` build embeds, and a job that downloaded it needs no Emscripten on `PATH`
(see [Sandbox runtime bundle](#sandbox-runtime-bundle)). After changing the runtime crate,
run `just sandbox-runtime` (or `just test`) before `just browser-test`. The browser
executable must be an absolute path. The shared workflow owns
tool acquisition. Rust repository tooling drives Chromium through its one CDP
connection using chromiumoxide 0.9.1, with default features disabled and no browser
downloader. Observed versions, checkout SHA and Cargo's emitted artifact paths are
printed.
One nextest run reports every real-browser case. The command selects every
`browser::` test of `maestro-test-browser` with `--no-fail-fast`: the driver's
own regressions, one test per inventory export and the report painting. Each
case is its own test, so the [hang limit](#test-time-budget) stops an export
that never completes and names it, and the run still reports every other case.
The command exits with that run's status.

The registered integration target is compiled with the workspace lock but never
loaded in the browser: its libtest startup would execute native tests. Runtime
execution uses an unregistered temporary cdylib pointing at the unchanged
`tests/integration.rs`, with `--cfg test` applied only to its root crate. Scratch
dependencies come from the current manifests. Packaging builds the Rust setup
command before fetching the checkout's locked dependencies. Either operation may
require network access on a cold cache. Rust repository tooling prepares the
scratch manifest and compares its resolved lock with the checkout lock. It resolves
workspace dependency aliases, combines
normal, WebAssembly and development features, anchors dependency and patch paths
to the checkout, and retains the library source. It copies the checkout's profile tables,
leaving out package overrides for crates the library never builds, so the library's
registry dependencies compile with the same settings as the registered integration target
and the second build reuses them. Native-only
dependencies stay outside the browser library. Preparation copies `Cargo.lock`
and `rust-toolchain.toml` and links the authored test directory.

The driver's `browser-export-setup` operations use the existing `development`
command. `binding-version` checks the observed CLI version against the single
locked `wasm-bindgen` package. `prepare` takes a scratch directory; `check-lock`
takes that same directory after offline Cargo resolution. Missing or malformed
manifest/lock data fails preparation. Lock errors and manifest parse errors include
the filename. An inherited dependency missing from the workspace fails with its
alias. External identities include name, version, source and checksum; any addition
fails before compilation. The authored manifests
and lock stay unchanged. Build outputs remain in Cargo's resolved target directory.

Bindings use `--target web --split-linked-modules` to retain local snippets,
relative fixture imports and native module identity. The `browser-exports`
nextest setup builds the package once per run, before any test that reads it.
Cargo builds the setup binary with the crate's tests. The binary writes the
package to `browser-exports` under Cargo's target directory, replacing the
previous run's copy. If setup fails, nextest prints its output and stops before
any browser test runs. The crate's build script generates one test per name in
`exports.json`, `browser::exports::<family>::<export>`: 27 execution exports,
70 loading exports, 189 contribution exports and 165 canvas exports. Each test
serves only the packaged output on an ephemeral loopback port, launches its own
browser and runs its export in a fresh page, with a disposable browser context,
page lifecycle and policy-error accounting. Nextest runs these tests in
parallel like any others. A missing or non-callable export fails its own test.
`browser::exports::every_built_export_is_in_the_inventory` checks the other
direction: every function the built module exports, apart from the binding
initializers `default` and `initSync` and generated classes, must be in the
inventory, and none is called. Packaging also
builds the two runtime-module fixtures, each from its own manifest and lock with
its own binding glue, into a copy of the fixture package.
It builds the prototype sandbox runtime the same way, with classic-script glue
(`--target no-modules`), for the bootstrap qualification exports; the
[sandbox runtime contract](specs/browser-sandbox-runtime.md#bootstrap-qualification-receipt)
records what they measure. Contribution
exports run on a page served under a content security policy without
`unsafe-eval` and receive the page's `url => import(url)` as their module import.
Canvas exports run on a page that holds only an 800 by 600 `#canvas` at the page
origin, with no body margin, because they compare canvas points with page
coordinates.
The setup hashes the host bundle once and exports the digest as
`MAESTRO_BROWSER_EXPORTS_DIGEST`. Each export test hashes the bundle again after
the export and fails when the two differ. Nextest prints each export's `PASS`,
`FAIL` or `TIMEOUT` under its test name, and its summary counts every case. `PHASE` records identify loaded
pages, resolved exports, pointer dispatch and pointer acknowledgments. Nextest
shows them for a failing or timed-out export, so an interrupted export keeps its
last observed browser phase.

After each native sandbox click completes, the driver checks the selected
fixture document for a fresh trusted click whose composed path contains its
`#target`. Missing, untrusted or wrong-target delivery fails with
`native click did not reach mounted target <id>` before waiting for the pointer
message. An isolated frame receives its input through its own browser session:
the driver scrolls `#target` into that document's view immediately, then sends
move, press and release there. A frame that shares its parent's renderer uses
that renderer's native content quad. The driver releases lookup handles after
success and failure, and retains attached renderer sessions only for the page's
lifetime. The driver tracks the default execution context of every
automatically attached iframe, including frames whose scripts are already
running when they attach. It registers the binding script as a
new-document script that also runs immediately, and sends no evaluation that needs a
default context, because a frame that loads from a URL attaches before its document
commits and has none yet: it runs the script at commit, and a `srcdoc` frame
runs it at once. The driver serves host calls one at a time. A call that arrives
while another is being served, or while an iframe is still being set up, waits,
and waiting calls run in arrival order once the driver is free, ahead of any
call that arrives while they drain. A burst of calls, such as the phase call
each synthetic click makes, therefore never nests one call inside another. A
session the caller attaches itself stays with the caller. An iframe removed
during its own setup is reported where a call uses it, and a host call whose
caller is already gone gets no reply and fails nothing else; page errors,
crashes and console panics raised during that setup still fail the command.
Browser events route by session; export, binding and input operations wait for
their own completion signals without command deadlines.

The launch tells Chromium that this desktop has a hover-capable pointer, so
media queries such as `(hover: hover)` behave like they do on a real desktop.
Pages keep touch emulation off; only an export that asks for touch input turns
it on. Every page draws at a device pixel ratio of 1.25. Chromium snaps font
metrics to device pixels, so a recorded formula box only reproduces on the same
pixel grid. `crates/maestro-web/tests/fixtures/markdown/math.json` holds those
boxes as measured from the reference KaTeX 0.16.45 in a standards-mode page, in
an 800 by 500 viewport at a ratio of 1.25, with the container typography the
replay sets before it measures. A re-record uses those same inputs, and the
replay compares every box to 1/64 of a pixel. A navigation
waits for the committed main frame's own default
execution context before it evaluates readiness, whichever side of the navigate
reply announces it.

The driver launches Chromium with `--no-startup-window`, so the only pages in
the browser are the ones the driver opens. Each switch a test supplies must be
written `--key` or `--key=value`; anything else is refused before launch, and
repeated `--disable-features` switches merge into one list.

Missing exports, synchronous throws, rejected promises, unexpected page errors,
page crashes and unexpected browser disconnection fail the export's test. An
acknowledged browser close or a lost connection ends command admission: later
operations, including page creation, fail immediately with a connection-closed
error. Script arguments in the browser fixtures are JSON-encoded before evaluation
so quotes in embedded document scripts remain data in the containing page.

Browser tests install their page hooks and observers from Rust inside the test
module, not from JavaScript modules. The `browser_fixture_support::Patch` guard
replaces one page property and restores its own previous definition when dropped.
It has four forms: a method (through a proxy whose `apply` trap hands the Rust hook
the original function, the receiver and the arguments), a getter, an accessor or a
value. Test-owned observers, such as the contribution page's mutation recorders,
selections, animation seeks and frame intervals, call the page's own objects
through `js_sys` and `web_sys`.

Content the product runs inside an isolated frame is Rust test data too, kept as
string constants beside the tests that hand it to the product. Supplied programs and
provider runtimes sit in `sandbox_program_inputs.rs`, which the program witnesses and
the compiler tests share. Authored documents sit in
`sandbox_loading_support/programs.rs` and `sandbox_loading_support/routes.rs`, and
the URL bootstrap in `sandbox_execution_support/mod.rs`. The bootstrap and the routing
URL document stand in for the page a host serves at its sandbox URL. Like the
driver's served `/sandbox.html`, the bootstrap writes a load message's code only when
the frame's parent sent it.

The raw console witness builds its native operands in Rust and posts them into an
artifact document, which posts each one back to its parent as its own message.

The completion witness keeps its observer as a supplied provider, the `PROBE`
constant in `native_html_artifact_browser.rs`. It records where the completion signal
runs, the document's readiness, syntax errors and the body once loading ends, so it
must be in place before the document's own scripts. Each sandbox document runs in its
own renderer, which the browser driver learns of only after the document has run;
only a provider runs early enough.

The Markdown corpus needs no page script. `markdown_support/projection` does the
comparison in Rust: it projects the shown document and the recorded reference
markup into one neutral tree, compares that tree with the recorded semantic
trees, resolves each formula through the recorded catalog, measures a mounted
formula's typography at the recorder's settings, and reads the painted roles from
computed styles. `install` publishes `root`, `diff` and `paintFailures` on
`globalThis.maestro` for the authored one-line checks, so no check imports or
evaluates a script first.

One deliberate policy-negative export,
`inline_programs_preserve_text_policies_and_order`, must produce exactly one
`TypeError` with message `window.complete is not a function` from `about:srcdoc`.
Its ordinary assertions still run; absent, repeated or other errors fail. Error
accounting stays attached until the page closes. Each test closes its browser and
server and removes its disposable profile on both outcomes; a cleanup failure does
not skip the other resource or replace an assertion failure.

The driver regressions, the export tests and the report test share
`maestro-test-browser`'s one integration target. This test-only crate owns the
browser stack and reuses
`maestro-tooling` for preparation, locked metadata and asset copying. Hakari excludes
it from traversal, so ordinary development commands never build the browser stack.
These tests carry the reason `runs in the pinned Chromium job`;
ordinary native runs leave them ignored and run the browser-independent input,
resource and pixel checks. The command selects only the `browser` module, keeping
controlled-child tests out. Run every browser case, or one export by its test
name, with the same supplied executable; the `browser-exports` setup runs first
whenever an export or the report is selected:

```sh
just prepare-tests
cargo nextest run --locked -p maestro-test-browser \
  --run-ignored only -E 'test(/^browser::/)'
cargo nextest run --locked -p maestro-test-browser \
  --run-ignored only -E 'test(=browser::exports::loading::<export>)'
```

The tool report page is a second witness in the same run, the test
`browser::tool_report_paints_from_its_verified_assets`. The Rust driver
serves the committed report fixture and answers the page's two pinned library
URLs from cached copies in `browser-analysis-assets` under Cargo's target
directory, whose SHA-256 digests it checks. A missing copy is fetched
with the existing three-attempt budget; a mismatch fails. Any other request outside
the driver's loopback origin is aborted and fails the test, checked after
navigation and again after the last observation, so the bytes the page executes
are the bytes the cache was verified with. The driver then observes what the
page paints: a row click must change the selected summary and the canvas bitmap,
a recorded `</script>` payload must stay inert text, and bar fills, chart label
text, heading glyphs, summary text and row borders must show the brand roles.
Canvas bands and screenshots are sampled as pixels, and a chart's own animation
completion is the render signal, so no test here waits on a clock. The native
report test holds the fixture byte-for-byte, so a template change has to move
both.

The canvas family covers every export that reads the page's `#canvas`: the
session-header, reading, PDF, Markdown and SVG artifact, artifact panel,
spreadsheet, text and image, generic file, tool renderer, offline tool card,
expandable-output, session-link and session-download exports. The driver's
inventory test fails when an export in those source files is outside the canvas
family. Transcript execution is tracked separately
in [the transcript test issue](https://github.com/Orchestration-Maestro/maestro/issues/801).
Browser execution is separate from native tests and coverage. `.github/ci.toml`
requests the shared Chromium job with
`browser_test_crates = ["maestro-app", "maestro-web", "maestro-test-browser"]`.
The job runs for merge groups and main pushes, and a manual `Browser proof`
dispatch runs it on a named ref; pull requests and advisory-only runs skip it.
Native package selection and the coverage floor stay unchanged.
See the [shared caller contract](https://github.com/Orchestration-Maestro/maestro-rust-workflows/blob/v0.19.14/docs/caller-checks.md#chromium-browser-tests).
A local pass is not hosted-CI proof; activation requires a successful shared job.

### Screenshot evidence

Eleven canvas exports, three contribution exports and two loading exports check the
pixels the page paints. Each one draws a state and awaits
`support::reading_paint::observe(scene)`, which calls the page's
`readingPaint(scene)`. The driver answers the call: it captures the visible page
with `Page.captureScreenshot`, decodes the image with the browser's own decoder
and keeps the pixels in the page, so only counts cross the protocol. It then
changes only the decoration under test, captures again and restores the exact
authored `style` attributes. A role pixel is an opaque pixel with exactly the
role's RGB channels, the same rule the tool report witness uses; the Markdown
syntax and brand scenes accept channels within 1 of the role's, because text is
antialiased. A
scene that counts a role by name reads the colours the export publishes first with
`support::reading_paint::publish_colors`. The call resolves `true` when every
check holds. Otherwise it rejects with the failed check's message, which fails
the export and the job:

| Scene | Export | What the screenshots must show |
| --- | --- | --- |
| `neighbors-header`, `neighbors-generic`, `neighbors-error` | `header_trailing_spacing_precedes_following_widget`, `generic_guidance_is_literal_selectable_and_localized`, `reading_frame_neighbors_keep_existing_paint` | Text paints inside the root: making it transparent changes pixels. Removing the reading frame's shadow changes none. |
| `neighbors-artifact` | `reading_frame_neighbors_keep_existing_paint` | The padding paints the caller's background role, the page shadows paint, and the frame gains no outer shadow. |
| `viewport-0` to `viewport-8` | `reading_viewport_keeps_content_and_occluders_clipped` | The full viewport keeps its placement. Content, the outside sentinel and the outer decoration stay inside the viewport, the ancestor clip, the canvas and the control holes, and native hit testing agrees. |
| `export-roles` | `export_rendering_uses_the_selected_brand_roles` | Each export element paints its brand role inside the final clip, for both packs in both modes. Borders paint exactly their role. Hover, expansion and underline change pixels. |
| `offline-tool-card-layout`, `offline-tool-card-roles` | `tool_card_image_and_diff_layout_uses_native_content`, `tool_cards_use_selected_brand_roles_without_changing_custom_html` | The decoded image and the diff text paint. Card roles paint, output hover changes pixels, and supplied spaces keep their background. The browser draws no underline on a run of only spaces, so the underline is checked on the export's text. |
| `control-focus` | `artifact_controls_dom_paint_tokens_and_keyboard_actions` | The control that keyboard focus reached paints the accent ring over more than half of each 2-pixel edge band, so no edge is missing or clipped away by its group. |
| `console-rest`, `console-hover`, `console-focus` | `artifact_console_dom_expansion_copy_and_follow` | In both modes, the summary and ordinary lines paint the muted role and error lines the error role. Under the native pointer, the summary paints the text role. The follow control that keyboard focus reached paints its accent fill and on-accent glyph, and its accent outline covers more than a quarter of the straight band along each edge. These scenes never move the pointer or scroll, so an element outside the visible page fails. |
| `console-collapsed`, `console-expanded`, `console-copy`, `console-copied` | `artifact_console_dom_expansion_copy_and_follow` | The glyph's ink, the pixels that change when the glyph is hidden, has the right shape: a collapsed summary paints a right chevron (taller than wide), an expanded one a down chevron (wider than tall), the idle copy control a square copy icon and the copied control a check mark wider than tall. |
| `thinking-rest`, `thinking-hover`, `thinking-expanded` | `thinking_dom_paints_brand_roles_and_disclosure_motion` | In both modes, the reasoning header's label and chevron paint exactly the muted role at rest and the text role under the native pointer: repainting either in that role's colour changes no pixel, while making it transparent changes some. At rest the chevron's ink is a right chevron, and once expanded a down chevron. |
| `word-page` | `docx_artifact_dom_styles_and_safe_targets` | In both modes and both widths, the visible part of the first page paints white over more than half its pixels, and its text paints black ink, pixels with no channel above `0x60`, for every package with text. The limit allows for thin antialiased glyphs that never reach pure black. Dark mode's brand text colour is light, so text that loses its black fails there. |
| `html-source`, `html-source-again` | `html_artifact_dom_keeps_frame_across_mode_changes` | In both modes, the shown listing paints the background role over more than half its box, and hiding its text changes pixels inside the bounded body and none outside it. After the listing is hidden and shown again, it paints exactly what the last `html-source` capture showed, so it kept its scroll position. |
| `markdown-hover` | `markdown_hover_keeps_nontext_paint` | Moving the native pointer onto the link changes the link text's pixels and none of the linked image's, and leaves the corner pixel of the inline-code background as it was. |
| `markdown-syntax` | `markdown_native_paints_all_syntax_roles` | In both packs and both modes, each highlighted token class paints pixels of the role its theme token names in `brand-roles.json`. |
| `markdown-brand` | `markdown_native_paints_selected_brand_and_loaded_math_fonts` | In the default pack's two modes and the alternate dark pack, with and without thinking, the paragraph (muted while thinking, text otherwise), link, inline code, quote border, table header, formula error and listing border paint pixels of their roles inside the reading viewport. |
| `link-hover` | `copy_link_native_slot_composition_preserves_reading_selection` | No screenshot: the driver moves the native pointer onto the message text, so its hover listener sees a trusted event. |

Each passed check prints `PAINT <export> <scene> <check> pixels=<count>`, so the
job log records the paint every check measured. The driver regression
`reading_paint_fails_when_the_painted_output_differs` tests the observation
itself: painted text passes, while transparent text, an outer frame shadow and an
unknown scene each fail. `control_focus_requires_an_unclipped_accent_ring` does
the same for the focus scene: an inset accent ring passes, while a missing ring,
an outer ring, a group that clips one edge and a page with no focused control
each fail. `console_scenes_require_their_painted_roles` covers the console
scenes: lines, hover and focus that paint their roles pass, while a line
in the wrong colour, a hover that keeps the resting colour, a missing outline, a
pointer away from the summary and focus away from the follow control each fail.
`console_glyph_scenes_require_their_painted_shapes` passes each glyph scene only
for its own glyph and fails a glyph that paints nothing.
`thinking_scenes_require_their_painted_roles_and_chevron` fails a label or chevron
in another colour, a pointer away from the header and a chevron turned the wrong
way.
`word_page_scene_requires_a_white_page_and_black_text` fails a grey page, grey
text and a missing page. `html_source_scenes_require_covered_clipped_and_unchanged_paint`
fails a transparent listing, a listing that paints outside its body, a changed
scroll position and a missing earlier capture.

Fonts are checked by measuring text. In the console export's log box, eight
narrow letters and eight wide letters take the same width, as they do in the
brand mono stack and not in the body font. The Word export awaits the `load()`
of the face its `embedded-font-loaded` package embeds, then requires the text in
that face to measure a different width from the same text in a family the page
lacks, so a face that fails to load cannot pass.

Every package in `tests/fixtures/native_document_artifacts.json` is also a
`.docx` under `src/word_view/fixtures/docx/`, and a native test holds the
two byte for byte. To change a package, edit the `.docx` and copy its base64
into the JSON. Another native test decodes the embedded face and compares it
with the font file it came from.

### Prove a browser test on a branch

Run the Chromium job on any branch, tag or commit, without the merge queue:

```sh
gh workflow run browser-proof.yml --ref main -f ref=<branch>
gh run list --workflow browser-proof.yml --event workflow_dispatch --limit 5
```

`.github/workflows/browser-proof.yml` calls the shared workflow at the release the
full suite pins, with `browser-only: true` and the `ref` input as the checkout. Only
the declaration check and the Chromium job run. The workflow file is read from
`main`; the test code and the `browser-test` recipe come from `ref`. Dispatches share
one queue: a newer one replaces a queued proof and never cancels a running one.
This repository keeps the default concurrency key the caller contract requires, so
a proof joins the shared `main` group and names its artifacts `root`. A proof
restores the compile cache but never publishes it. Its call grants
`actions: write` only because GitHub checks the shared pruning jobs against the
grant when the run starts; those jobs skip on a dispatch.

The job saves the recipe's output as `browser-test.log` in the artifact
`browser-test-root-<attempt>`, whether the recipe passes or fails:

```sh
gh run download <run-id> --name browser-test-root-1 --dir proof/red
```

To prove a test red and then green:

1. Push a branch that breaks one assertion of the test. Keep the test registered and
   its selection unchanged. Note the commit SHA and the test's name.
2. Dispatch the proof for that branch. The run must fail at that assertion, not in
   setup or compilation, and still report every other case: nextest's summary
   counts every browser test.
3. Restore the assertion on the branch under review and dispatch again. The run must
   pass.
4. Save the two `browser-test.log` files as `<test>-red.log` and
   `<test>-green.log`, with both run URLs, both source SHAs and the shared release.
5. Delete the throwaway branch.

Use this before a pull request joins the merge queue, whenever its change touches a
browser test or the code one exercises: pull requests skip the Chromium job, and a
failure found in the queue costs a full queue cycle.
See the [manual browser proofs](https://github.com/Orchestration-Maestro/maestro-rust-workflows/blob/v0.19.14/docs/caller-checks.md#manual-browser-red-and-green-proofs)
in the shared caller contract.

## Extension guide lock

This repository carries no examples tree or guide fixture, so `just example-index`
refreshes only the example targets of the component crate here. In a workspace
that carries the extension guide, run it after changing dependencies to
regenerate the extension index, example targets and the standalone extension
guide's `Cargo.lock`.
`just check` runs `just example-index-check`, which reports stale generated files
without changing them and names the regeneration command.

The generator seeds a disposable copy of the guide project with the workspace
`Cargo.lock`, then lets Cargo resolve offline. This retains the workspace's exact
versions and uses the actual guide manifest and guest dependencies, rather than
maintaining a second dependency list. The clean-folder guide test still builds
with `--locked`. Populate the Cargo cache first if offline resolution reports a
missing package; generation does not fetch dependencies.

The tooling tests cover stale-lock detection, regeneration and resolver failures.
Their path-only dependencies do not prove that resolution retains the workspace's
registry versions: a matching Cargo cache can produce the same lock without the
workspace seed. The shipped-index check compares the committed lock with the
resolver's output, but it is not an independent test of workspace seeding.

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

It leaves tokio out for the same reason: members request its `test-util` feature
in their dev-dependencies only, and the unifier would otherwise carry the pausable
clock into every production build. Each member declares the tokio features it
uses, and two conventions checks cover part of that. One runs
`cargo tree --target all -e features,normal,build -i tokio` for the `maestro`
binary and fails if `test-util` appears. The other is part of the workspace
conventions check: it fails when a member's production code spells `select!`,
`join!` or `try_join!` from tokio, by path or import, or the `main` or `test`
attribute from tokio, by path, without declaring `macros`.

Regenerate after changing a dependency, its features or `Cargo.lock`:

```sh
cargo hakari generate
cargo hakari manage-deps
```

After adding a member, first add `maestro-dependencies.workspace = true` to its
`[target.'cfg(not(target_arch = "wasm32"))'.dependencies]` table, then run both
commands. `just check` runs `cargo hakari verify`, which checks feature
unification within the configured scope and exclusions. It does not prove the
generated section is current or that every member declares the dependency; confirm those with `cargo hakari generate --diff` and
`cargo hakari manage-deps --dry-run`, which exit nonzero when work remains.

## Extension components

```sh
just extension-author-component
```

Builds the example extension as a `wasm32-wasip2` component and adds that target
with `rustup` when it is missing. See [writing an extension](extensions/guest.md).

## Source invocation

```sh
just --justfile /path/to/maestro/justfile run-source --help
just --justfile /path/to/maestro/justfile run-source --no-env --help
just --justfile /path/to/maestro/justfile run-source-windows --NO-ENV --help
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
just dev
just models-dev-compile
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
one resolves against `crates/maestro-tooling`. Each line starts with the seconds
since the first record in the process, and every recipe records its start and its
exit status, so one file shows where a slow attempt spent its time. The harness
does not capture commands whose output a test reads only after they exit. A
forced-kill test checks both directory routes and verifies that child lines survive
termination while still reaching stderr and the watch consumer. There are no exact
build counts or negative observation windows. Their watch processes share a
test-owned process group, stopped on completion or panic. Before starting a
watch-test process group, the harness directs its LLVM profiles into that fixture's
target directory, outside the merged coverage profile set, because cleanup can
forcibly stop its descendants.

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
The terminal redraw log, the width crash report and the cost, usage, transcript and
tool reports all follow this one directory.
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

Use the existing [brand theme generator](theme.md#brand-pack) to try a pack without
changing shipped files. Run this from the checkout root:

```sh
scratch=$(mktemp -d)
cargo run --locked -p maestro-theme --example brand_themes -- \
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
The examples come from the tree `just setup` fetches from maestro-packages; without
it the command fails and names the missing `examples` folder.
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
| RPC mode | [`maestro-rpc/src`](../crates/maestro-rpc/src/lib.rs): the session records, the connection runner, the client and the standard-stream adapter | [Daemon protocol](daemon-protocol.md) |
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
just test
just test-offline
just test -p maestro-tooling maestro_source_without_switch_preserves_environment
```

The broad test recipes run cargo-nextest, then Cargo doctests only after success.
They forward literal arguments to both phases; use options accepted by both,
such as name filters and `-- --ignored`, `-- --exact NAME` or `-- --nocapture`.
`just test` selects the whole workspace. Name a package with `-p NAME` or
`--package NAME` and both phases build and run that package alone, with its
dependencies. A package without a library, such as `maestro`, has no doctests:
its selection runs nextest, and the doctest phase passes without running
Cargo's doctests.
An unmatched ordinary-test filter fails rather than passing an empty selection.
Use direct `cargo test --doc` for doctest-only runs and direct `cargo nextest run`
for runner-only flags or target selection. `tui-test-ansi` runs only nextest.

### Shared test support

Keep one owner for a reusable fixture inside its crate. When unit and integration
tests need the same fixture, put it in `tests/support/` and include that file from
both test programs. A test-only self alias lets the file name its owning crate
consistently in either program; it adds no production export.

For example, the models tests include
[`oauth_admission.rs`](../crates/maestro-models/tests/support/oauth_admission.rs)
from `src/tests.rs` and `tests/integration.rs`. Each program keeps its own registry
admission lock and resets its registry on entry and drop. The same layout owns
native peer reads, process readiness polling and image-enabled terminal fixtures.
Helpers used only by sibling unit modules stay beside those tests, as the daemon's
terminal fixture does. Chat callback recorders and web pointer events likewise
stay in their existing integration-test support modules.

Use the shared fixture without changing the caller's assertions or adding a
production feature for test access.

Keep a crate's `examples/` directory for examples written for users. Put a program
that exists for tests, checks or test preparation (a native test child, a fixture
component, a browser compile witness, a terminal debugging program) in
`tests/support/`, registered as an `[[example]]` target with an explicit `path`, as
the models crate's browser witnesses are. Put a program that maintains repository
files or reports on recorded data (a generator, a report tool) in the crate's
`scripts/` directory, registered the same way. Both keep the `[[example]]` kind, so
`cargo run -p <crate> --example <name>` and `target/<profile>/examples/<name>` stay
the same.

An example with `test = true` gets its own test executable, which links the whole
crate graph again. Leave the flag off. `count_prompt_tokens` still carries it until its tests move
into the integration program. Cargo still builds and lints the example with the
crate's other targets, and `cargo run --example` runs it. Test the
behaviour it shows in the crate's integration program, as the `maestro-app` SDK
examples do with the scripted workspace in
[`sdk_sample.rs`](../crates/maestro-app/tests/support/sdk_sample.rs).

### Large fixtures

A fixture larger than 1 MB (1,000,000 bytes) is read when the test runs, not compiled
into it. `include_str!` and `include_bytes!` copy the whole file into the test
executable, so every link carries it and any edit to the data rebuilds the test unit.
Join the crate directory with the fixture path instead:

```rust
fn cases() -> Vec<Case> {
    let path = format!("{}/tests/fixtures/cases.json", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(path).expect("recorded cases");
    serde_json::from_str(&text).expect("decodable recorded cases")
}
```

A site that sits in a `const` or `static` becomes a function like this one. The
`fixture_includes` check rejects a larger compiled-in fixture in test code, naming the
including file, line, fixture and size.

### Library test targets

A library with no tests in its `src` still builds an empty test executable on every test
build. Set `[lib] test = false` in its manifest to stop building it, and remove the flag
when a test moves into `src`.

The conventions check keeps the flag and the sources in step. It reports a library that
holds in-source tests and disables the target, and one that holds none and leaves the flag
off. In-source tests are a test module, a `tests.rs` or `tests/` source under `src`, a
`#[cfg(test)]` gate or a test attribute. The integration target at `tests/integration.rs`
is separate and runs either way.

### FTP peer support

The models tests share the scripted peer in
`crates/maestro-models/tests/support/ftp_server.rs`. Library tests also load
`ftp_server_timing.rs` for `Peer::reached` and `Peer::written`; the integration
program uses only the common peer. Check both programs for unused support:

```sh
cargo clippy -p maestro-models --all-targets --locked -- -D warnings
```

### TLS fixture certificates

The provider tests trust fixed certificates under
`crates/maestro-models/tests/fixtures/`. They are committed data, and no script or
certificate library issues them. Each one expires on 2126-10-18, so none of them
lapses during a test run. The three server-side certificates start on 2019-06-01,
before 2019-07-01, because Apple's TLS policy refuses a server certificate issued on
or after that date that is valid for more than 825 days. The client pair starts on
2026-10-17.

| Files | Subject | Made as |
| --- | --- | --- |
| `socks-tls/cert.pem`, `cert.der`, `key.der` | `CN=destination.invalid`, names `destination.invalid` and `localhost` | Self-signed server certificate over the committed key. It is its own trust anchor. |
| `cloud-authentication/server-ca.pem` | `CN=Controlled Maestro ADC server authority` | Self-signed authority, `CA:TRUE`, key usage `keyCertSign` and `cRLSign`. Its key is not kept. |
| `cloud-authentication/server.pem`, `server-key.der` | `CN=localhost`, names `localhost` and `127.0.0.1` | Server certificate over the committed key, signed by the server authority. |
| `cloud-authentication/ca.pem`, `client.pem`, `client-key.der` | `CN=Controlled Maestro ADC CA`, `CN=Controlled ADC Client` | Left as issued: the recorded chain hashes depend on them. |

The server certificates carry `serverAuth` and `digitalSignature, keyEncipherment`
with `CA:FALSE`, signed with SHA-256 and a random serial.

To replace the SOCKS certificate, issue a new self-signed certificate over the same
committed key with the subject and names above, then commit the PEM and DER forms
together. To replace the cloud server certificate, make a new authority key and
re-issue `server-ca.pem` and `server.pem` together, because the authority key behind
the committed `server-ca.pem` is not kept. Keep the 2019-06-01 start and the 2126
expiry on all three server-side certificates.

### Code scanning

Open the repository's **Security > Code scanning** page to inspect CodeQL alerts.
The workflow scans each push to `main` and runs every Monday at 04:17 UTC on
GitHub-hosted Linux. It covers Actions, JavaScript/TypeScript and Rust with the
default query suite. Pull requests and merge groups start no scan.

The Rust scan excludes `crates/*/tests/**` through its matrix configuration's
`paths-ignore` setting. Assertion diagnostics in those directories format
controlled fixture values, not credentials. Keep those assertions and
diagnostics intact. Rust files outside those test directories,
including production code, examples and unit tests under `src`, retain the default
queries. Actions and JavaScript/TypeScript scans have no path exclusions.

CodeQL is advisory: no ruleset requires its checks. Scan failures and alerts
remain visible in GitHub but do not block the merge queue.

### Slowest-first test order

Nextest starts the slowest tests first inside each shard. The tests that took more
than 30 seconds in the merge-group run below hold distinct priorities in
`.config/nextest.toml`, 100 down by duration. The cold browser build always holds
100. A test that falls under 30 seconds gives up its priority and its table row.
Other tests keep priority 0. These are
[nextest's built-in priorities](https://nexte.st/docs/configuration/test-priorities/),
not a scheduler or a shard planner. They change order within each shard, not the
count partitions (16 in CI, declared as `test_shard_count` in `.github/ci.toml`), test
membership, retries, time limits or coverage policy.
Existing timeout comments retain their older CI maxima; priority ranks use the
measurements below, not timeout ceilings or diagnostic phase timings.

The measurements come from [merge-group run 43212429686](https://github.com/Orchestration-Maestro/maestro/actions/runs/43212429686),
revision `288c311fa4d7b9890050fcc96a352c5150cc702b`. Its nextest spans in shard order
(1 through 16), in seconds, are:

```text
213.109 / 131.345 / 80.360 / 58.752 / 74.250 / 62.847 / 44.174 / 44.259
48.030 / 46.890 / 34.224 / 51.338 / 35.220 / 35.749 / 40.009 / 40.175
```

The slowest shard is **213.109 seconds**; the spread is **178.885 seconds**.
The final successful `(binary, test)` identities total **16,961**, with no
identity shared by two shards. Retries do not add identities to this accounting.
Setup-script durations are excluded from per-test ranks, but included in the shard
spans above. One test, the browser example served from rebuilt external assets,
crossed the 30-second flag; no test reached the 120-second hang limit.

The fast tooling regression
`nextest_priorities::measured_priorities_order_tests_without_changing_shards`
runs nextest against controlled bodies with the real tooling overrides. One
worker proves the cold browser build starts first, then the next ranked tooling
test, before the unranked tests, including one that held a priority in an earlier
measurement. The split is fixed at 16 lanes, independent of CI's
`test_shard_count`. It compares every partition's membership with and without
priorities, checks an exactly-once union and keeps an intentional selected failure
visible. It does not run the expensive browser bodies.

#### Measured successful test durations and priorities (seconds)

The cold browser build takes every core, so it holds priority 100 whatever its
duration. Nextest starts tests in priority order and holds the queue until the
next one fits: a test ranked above the cold build would run alone while the
other cores sat idle.

| Priority | Package / test | Seconds |
| ---: | --- | ---: |
| 100 | `maestro-tooling` / `browser_author_build::browser_author_bootstraps_from_empty_build_state` | 29.597 |
| 99 | `maestro-tooling` / `browser_author_build::browser_example_development_serves_rebuilt_external_assets` | 31.661 |

#### Refreshing priorities

To refresh the ranks:

1. Download the latest successful merge-group logs with
   `gh api repos/Orchestration-Maestro/maestro/actions/runs/RUN_ID/logs > logs.zip`.
   Record the run URL, revision, shard summaries and complete per-test durations.
2. Read final successful `PASS [ Ns] (completed/total) binary test` rows, including
   successful retry rows. Exclude `SETUP PASS` and final-summary repeats. Check
   unique identities across shards against the summed summaries and eligible
   inventory; account separately for intentionally ignored probes and retries.
3. Sort the tests over 30 seconds by descending duration. Drop the old priority
   fields, give the cold browser build (`threads-required = "num-cpus"`)
   priority 100 and the rest distinct descending priorities from 99. Leave every
   other test at 0. Order tied durations by binary, then test name. Never refresh
   a priority by changing an exception limit or selection filter.
4. Update this evidence table and run the focused tooling regression:
   `cargo nextest run --locked -p maestro-tooling --test integration -E 'test(measured_priorities_order_tests_without_changing_shards)'`.
   If the slowest tooling identity after the cold build changes, update the
   controlled bodies and expected order to match; the cold build stays first.
5. Record the next merge-group run's slowest span and spread on #3613, with its
   inventory and revision. Explain setup, serialization or source changes rather
   than treating per-test duration sums as shard wall time.

### Test time budget

Nextest flags a test as slow every 30 seconds and kills it after four periods,
at 120 seconds. The 30-second flag is the budget: fix a slow test at its cause,
and do not approve one on headroom. The 120-second kill is a hang limit, not a
budget. It stops a hung test and leaves a busy shared runner room to finish a
test that normally runs well inside it. The default profile sets both:

```toml
[profile.default]
slow-timeout = { period = "30s", terminate-after = 4 }
```

The Chromium job runs under the same limit. `just browser-test` is one nextest
run in which every browser export, the report page and each driver regression is
its own test, so an export whose page never answers is killed at 120 seconds,
its `TIMEOUT` line names it, and the run still reports every other case. The
driver itself sets no command deadlines. See
[browser integration qualification](#browser-integration-qualification).

A test that needs more than 120 seconds gets an exception in
`.config/nextest.toml`. It must select exactly one test, or one shared fixture
group, and name the measured work and an open issue that will remove it:

```toml
[[profile.default.overrides]]
filter = "package(=maestro-example) & test(=component::loads)"
# Measured: 143.549s; cause: compiling the component fixture; #3589
slow-timeout = { period = "30s", terminate-after = 5 }
```

Keep `period = "30s"` so the test still prints SLOW every 30 seconds, and set
`terminate-after` to the fewest whole periods that cover the measured duration.
If a hosted run killed the test, record the kill time as the lower bound and say
so in the `cause:` text. Keep the `Measured:` seconds (over 120), `cause:`
description and `#NNNN` issue in comments immediately above `slow-timeout`. Each
exception needs its own comment; another override's comment does not apply. A
test that finishes within 120 seconds needs no exception.

The conventions check rejects a default other than the flag and hang limit, a
profile limit above the hang limit, a profile that changes the 30-second
period, an exception that changes the 30-second period or that the hang limit
already covers, an undocumented exception and an exception whose
`terminate-after` is not the fewest periods covering its `Measured:` seconds.
It checks references offline; the author and reviewer verify with GitHub that
the linked issue is still open. The change fixing the slowness must delete its
override as well as close the issue. Each existing exception has its own issue,
and #3589 tracks them together.

The development profile uses `debug = "line-tables-only"` for workspace crates:
backtraces retain file:line for workspace code, without debugger variable views.
The test profile uses `debug = false`: line tables cost about 17% of the compile CPU
of a large test unit. Backtrace frames inside test binaries show function names only,
while a panic message still names its file and line. To read file:line in a test
backtrace, build that test with `CARGO_PROFILE_TEST_DEBUG=line-tables-only`.
Dependencies build without debug info in both profiles, so their frames show function
names only; this keeps target directories about a sixth smaller.
Both profiles also build these dependencies optimised; release builds are unchanged:

- `cranelift-codegen` and `regalloc2`: tests compile extension components, and an
  unoptimised code generator makes each compile about five times slower.
- `syn`, `proc-macro2` and `ra-ap-rustc_lexer`: the conventions tests parse and lex
  every workspace source.
- `sha2`: the test setup hashes the lock files on every nextest run, and an
  unoptimised digest makes its reused-directory path about three times slower.

Archived CI shards receive `TEST_ARCHIVE=1` and `TEST_PREPARE_OUTPUT` pointing to
`target/test-setup`.
The `test_prepare` command in `.github/ci.toml` is the development tool's
`prepare-test-setup prepare`. Shared CI runs it only when a selected package
matches a setup filter in `.config/nextest.toml` for one of the setups named by
`test_prepare_setups` in `.github/ci.toml`. The consumer packages live only in
those nextest filters. To give another package a prepared fixture, add it to the
matching filter; no CI package list needs updating. A change to the nextest
configuration runs the full test suite in CI.
Shared CI caches the bundle under the contents of the files that
`test_prepare_inputs` matches and restores it instead of preparing again. Those
patterns cover the `maestro-extensions-wasm`, `maestro-tooling`, `maestro-web` and
`maestro-models` builds, the independent guide project and every workspace crate
they depend on through a path dependency of any kind, plus the fixed inputs.
A change to any other crate keeps the cached bundle.
The preparation reads that closure from the workspace metadata, compiling nothing, and
records it in the bundle manifest, so a shard validates the same files without Cargo.
`ci_cache_key_names_exactly_the_files_the_preparation_reads` fails when a file the
preparation fingerprints is not matched or a pattern names anything else, so a new
dependency of a built crate needs its pattern. Markdown that crates embed only as
documentation changes no staged file, so no pattern names it. The command runs the existing
component-discovery example from a separate build directory, then stages the
component files, the compiled code the example left in the engine's cache, that built example,
the browser graph described under [Browser author build](#browser-author-build), the
registry units of the cold browser bootstrap described under
[Cargo state of tooling fixtures](#cargo-state-of-tooling-fixtures), the
development tool itself, the [prepared Cargo checks](#prepared-cargo-checks) and a content
manifest. The compiled code is the one cache
namespace the engine named for the example's modification time; preparation fails on a
namespace that does not end in that time or on more than one. The setup example's
`--cache-directory` mode reports the engine's selected cache directory as one absolute
JSON path without building or warming components. Preparation queries that directory once
on its host before seeding or warming; installation queries it again on the test host. A failed query,
empty reply or nonabsolute path stops setup. Host paths are not stored in the bundle.
`MAESTRO_CODING_AGENT_DIR` isolates preparation's cache under its target directory on every
platform, because the engine keeps its cache below the agent directory. Shared CI transfers
these files separately from the native test archive, so a shard does not need the
tool in its archive. Nextest runs the staged tool's
`nextest-setup author-component`, which checks source and artifact hashes, including every
compiled-code file, installs the staged example beside the extracted executables and writes
the component paths to `NEXTEST_ENV`. The setup then gives the executables one modification
time, as below, and installs the compiled code in the engine-reported cache under that time.
A copy that keeps an older file time than another executable therefore still lands in the
namespace the example and the tests read. It runs the example
on the prepared components, which finds every component compiled and starts no Cargo build
or compile pass. On every platform that cache belongs to the run alone: the setup uses
`run-agent` beside the profile directory as the run's agent directory, queries and fills
the engine's cache below it, and exports the directory as `MAESTRO_CODING_AGENT_DIR`, so the
tests read the code the setup warmed. The engine prunes its cache by age and size
from every process that uses it; with a private cache, other builds on the same host
cannot evict the warmed code while a long run is still going. Archived runs
require the bundle and fail without starting Cargo when it is missing or stale.
Other runs reuse an existing manifest, rejecting stale or incomplete files; without
a manifest they keep local auto-build. Archive strictness
requires the shared CI release that supplies `TEST_ARCHIVE=1`. The manifest records one
digest per artifact instead of one for the whole bundle. All components share one digest over
the WebAssembly closure of `maestro-extensions-wasm` (its dependencies, without the edges
gated to native targets), the lock entries of the registry packages that closure uses, the
part of the workspace manifest a release build reads, the Cargo configuration and the
toolchain. `cargo build` compiles the
development dependencies of the crates it builds and of no crate they depend on, so the
digest follows development dependencies for the built crates only. For every crate in the
closure it leaves out `tests/` and the unit test files (`src/**/tests.rs` and
`src/**/tests/`); for every crate it does not build, it also leaves out the
`[dev-dependencies]` tables of the manifest. A test-only change in a crate the components
only depend on keeps every component, while a change to its source or to another manifest
entry rebuilds them all. The digest reads the workspace manifest as `cargo build --release`
does: it keeps the resolver, `[workspace.package]`, `[workspace.lints]`, `[profile.release]`
with its package overrides, `[patch]` and any other table, and the `[workspace.dependencies]`
entries the digested closure manifests name. A change to the member lists,
`[workspace.metadata]`, another profile or an entry no such manifest names keeps every
component; any other change to the workspace manifest rebuilds them all.
Each component adds a digest of the example files Cargo reports it read, so a helper module
that several components share is part of each of them. The setup example and the development
tool, and the browser graph, each hash the closure of their own packages with the examples,
assets, locked dependencies and toolchain they read. A shard's `author-component` setup checks
the shared, component and tool digests, and its `browser-author` setup checks the browser
digest, so a change to the tooling or the application leaves the components valid. The variable
names in the bundle follow component discovery's own rule and fixed author and context pair.

`TEST_PREPARE_BASE` names the directories of earlier bundles in the platform's path-list
syntax (`:` on Unix, `;` on Windows). The preparation ignores empty entries, reads the
remaining directories in order and uses the first one it accepts; a single directory is a list
of one. A base prepared from the same shared digest, so the same toolchain, supplies every
component whose example files are unchanged: the preparation hands those to the setup example
through the component variables, and the example builds only the others in its one Cargo call.
The base's compiled code seeds the build's cache, and is dropped again when the example writes
another engine's namespace. A base that is missing, unreadable or prepared from other shared
inputs is skipped, and when none is accepted everything is built; the base directory may be the
output directory.
A local run reads the same list. With `TEST_ARCHIVE` unset and no prepared bundle, the
`author-component` setup accepts a base by the same digests, hands its setup example every
component that base supplies and seeds the base's compiled code in the run's own cache, so
the example builds only the other components. The tests read the supplied components from
the base directory, so the base has to stay in place until the run ends.
After a successful preparation, the command reports how many components came from the base
and how many it built, for example:

```text
components: 74 reused, 0 built
```

File timestamps do not affect reuse, but the staged example finds component
sources at the checkout path it was built in, so both jobs must use the same
checkout path, as shared CI does.

For a local check of that boundary, run:

```sh
cargo run --quiet --locked -p maestro-tooling --bin development -- \
    prepare-test-setup prepare "$PWD/target/test-setup" "$PWD/target/test-prepare"
TEST_PREPARE_OUTPUT="$PWD/target/test-setup" cargo nextest run --locked -p maestro-extensions-wasm
```

The development tool's `nextest-setup` command runs every nextest setup entry:
`author-component`, `fixture-sources`, `browser-author` and
`normalize-component-cache`. `.config/nextest.toml` names only these commands, and the
conventions check rejects a shell script anywhere under `.config/`. The
`author-component` entry runs the tool from `target/test-setup/development`: a shard
receives it in the downloaded bundle, and `just prepare-tests` installs it. `just test` and `just check` run that recipe first.
When you start nextest by hand on a package the entry serves and the tool is missing,
nextest prints the missing path `target/test-setup/development`; run `just prepare-tests`
once and start nextest again. Run it again after a change to `maestro-tooling`, as nextest
runs a staged copy that is outdated without a word. The tooling entries run
`debug/development` from the target directory, which every `maestro-tooling` test build
produces. On Windows the staged tool is `development.exe`; the configured command
resolves that native executable. Preparation also copies, hashes and restores the setup
example as `prepare_author_component.exe`. Linux and macOS keep extensionless executable
names. Cargo's `--example prepare_author_component` target name stays the same on all hosts.

A test that starts a setup child which reads a prepared-bundle input the test does not supply
clears it (`TEST_PREPARE_BASE`, `TEST_PREPARE_OUTPUT` and `TEST_ARCHIVE`) through
`maestro_test_process::without_prepared_bundles` from the test-support owner, so a bundle a
machine exports for CI cannot change what the test reads, in this crate or in any other. The
conventions check reads every test source that spells `nextest-setup`, so a setup child added
without that helper fails the conventions run. The check follows the helper directly or through
a function of the same source, reading the written syntax without expanding macros.

Every development command that runs a child exits with that child's status. A child ended by
a signal has no exit code, so the tool prints the native status on standard error, such as
`child ended by signal: 9 (SIGKILL)`, then exits with status 1. That line separates a killed
example from one that failed on its own.

With `TEST_ARCHIVE` unset and no prepared bundle, local runs build the example
automatically. Before guest-authoring tests, the `author-component` setup locates the active
artifact directory through its documented platform library search path. It runs
the `prepare_author_component` example that the test build put in that directory,
as long as the example is newer than its source, the shared build and component
discovery sources, the package manifest and `Cargo.lock`. Running it through Cargo instead would resolve features for
that one package and recompile the engine. When the example is missing or older, as
after a `--lib` or filtered selection, the setup builds it through Cargo in the
same target directory and profile before running it directly.
The setup fails if the directory cannot be resolved rather than building
elsewhere. It uses `NEXTEST_LD_LIBRARY_PATH` on Linux,
`NEXTEST_DYLD_FALLBACK_LIBRARY_PATH` on macOS and `PATH` on Windows, split with the
platform's own separator.
Before running the example, the setup's cache normalization (`nextest-setup normalize-component-cache`) gives executable
artifacts in the profile directory, `deps` and `examples` their newest existing
modification time. This advances older outputs without making them older than
their compiler inputs. On Unix it changes each time through a read-only handle and
never opens an executable for writing: macOS can kill the next launch of an
executable that was opened for writing after it ran. It skips an executable when
an exact linked library build is newer than it: advancing that binary would hide
a rebuild from Cargo. For the same macOS reason, preparation and setup replace a
file they stage or install instead of copying over it.
Cargo's `.fingerprint` records identify each direct and transitive library build
by its fingerprint and metadata hash, independently of dependency aliases.
A newer build of the same crate under another hash, including check-only metadata
from Clippy, does not outdate it.
A build script's run is a compilation input, not a linked build. Cargo names the
script's crate after its file, so `build = "builder/main.rs"` gives
`build_script_main`; normalization recognizes a run by its record kind instead.
An uplifted executable uses the records of its hashed twin: normalization matches
length and contents, so copied and hard-linked outputs work alike. Identical twins
with different dependency records fail rather than select an uncertain build.
Cargo names Windows MSVC binaries and examples without a hash, so they have no
twin; such an executable uses the one record of its target (`bin-<name>` or
`example-<name>`). Several records of that target that differ leave it without a
record.
A library's own test binary does not depend on that library's separate output.
Without an executable fingerprint, normalization conservatively treats any newer
library output as an input. If a recorded dependency build no longer exists,
its consumer stays stale. This can happen after cleaning dependency packages
and rebuilding with another feature selection while older consumers remain.
An empty library fingerprint hash marks a build Cargo has invalidated.
Normalization skips that build, keeps its consumers stale and advances unrelated
current binaries. Cargo can leave the hash empty after a failed or interrupted
rebuild. A malformed non-empty hash or dependency record fails setup with a
diagnostic. These records follow the pinned Cargo toolchain.
A run from a nextest archive (`TEST_ARCHIVE` set) skips the fingerprint check,
because nextest never rebuilds archived executables. A skipped test binary compiles
its components itself on first use. A skipped example would warm code that no test
binary reads, so normalization exits with status 3 and setup rebuilds the example
through Cargo before running it. If Cargo retains an outdated example, setup logs
one line and runs it anyway; a cache miss does not prevent preparation.
The Git-pinned engine includes the shared modification time in its cache
namespace, so the preparer and consuming test binaries share compiled code.
The example builds every exported component in one Cargo invocation, so Cargo
compiles them in parallel, warms their Wasmtime compilation cache
in the engine's cache (on Linux the run's own `component-cache` beside the
profile directory) on up to four threads and exports each path to the tests' processes.
Component contents, compiler settings and CPU features still determine cache
compatibility.
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
which joins a single-thread nextest group. To serialize those tests across runs
on one machine, set `MAESTRO_FIXED_PORT_LOCK` to the same absolute, writable file
path in each run. The `fixed-port` nextest wrapper creates the file if needed,
waits for an exclusive file lock and holds it only while each selected test runs.
Without the variable, the wrapper runs the test directly. Keep the wrapper's filter and
the `oauth-callback` group's filter together in `.config/nextest.toml`. Plain
`cargo test` does not use the wrapper; serialize fixed-port tests separately.

Browser author-build tests also join a single-thread group because Trunk shares
binding/optimizer intermediates; their file lock keeps plain Cargo tests safe.
Other tests remain parallel.

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
The conventions check names every test that reads `docs/` or `assets/` at
run time, through a `../../` path, a join onto the workspace root or a call to
`get_docs_path` or `get_package_dir`, and fails until
`test_inputs` routes that directory to the reading package. The
[workspace conventions](architecture.md#workspace-architecture-checks) read every
crate's manifests and sources, so `crates/` routes to `maestro-test-conventions`: a
change under any crate selects the conventions suite beside the changed crate and
its dependents.

### Test preparation in a consuming workspace

The development tool prepares the workspace it starts in. Install it once, start it
from a workspace, and it reads no directory of the tree it was compiled in. One
tool, one example and one bundle serve Maestro and a workspace that builds on
Maestro through git dependencies.

```sh
target/test-setup/development prepare-test-setup prepare target/test-setup target/test-prepare
target/test-setup/development nextest-setup author-component
```

Preparation keeps the running tool intact when it is already in the output
directory. `prepare-test-setup install` does the same. Staging from another
directory replaces the destination file instead of rewriting it.

Every Cargo command that preparation and the nextest setups start, the cold
browser seed builds included, runs on the toolchain rustup activates in the
workspace being prepared. A dependency that ships its own `rust-toolchain` file,
or a machine default that differs from the workspace pin, cannot change the
compiler. A `RUSTUP_TOOLCHAIN` the tool inherited stands, because rustup ranks
it above the workspace files. A machine without rustup runs Cargo as it is.

The tool takes its checkout from the workspace root above the directory it starts
in: the nearest `Cargo.toml` that declares a workspace. Start it anywhere inside the
workspace. A command that reads no workspace, such as `prepare-test-setup install`,
is started in a directory of its own and keeps it; one that reads a workspace
reports that it found none. Nothing reads the tree the tool was compiled in.

The crates it prepares are Cargo's workspace members, so a nested fixture
workspace and a crate the root manifest leaves out are not its crates. A package the
workspace only depends on is reached through Cargo's resolved metadata, whose
checkout sits outside the workspace.

Four things it needs are the workspace's own:

- **The component crate** is the member crate whose manifest holds the generated
  example region between `# example-index:start` and `# example-index:end`.
  `development example-index` renders into that crate, and preparation builds its
  examples from that crate's package; only Maestro's fixed components come from
  their own packages. Two marked members fail with the crates named. Without a
  marked member, `example-index` stops with the workspace named, and preparation
  skips the components and the setup example; the native fixture, the Cargo
  checks and the executable are still prepared. A bundle without a setup example
  cannot run `nextest-setup author-component`; that setup stops with a message
  naming the missing example.
  The crate's own fixture folder, `tests/support`, is optional: without it the
  index and preparation use the shipped examples alone.
- **The shipped extension folder** is `examples/extensions` beside the workspace
  manifest, when git does not ignore it: a checkout that `just setup` filled from
  maestro-packages ignores the fetched tree, so the tools never read it. Without it the index renders the crate's own example targets alone and
  writes no index page and no guide lock.
- **The independent guide** is `tests/fixtures/hello-extension` under whichever
  member crate carries one, and it builds on the guest SDK package the workspace
  resolves. A workspace without the fixture skips the guide build and its lock.
- **The programs** preparation runs: Cargo builds them from the workspace when it
  holds their packages. A workspace that holds them only as git dependencies
  installs them once per preparation instead, with
  `cargo install --locked --git <source> --rev <locked commit>` for
  `maestro-test-process --example native_fixture`,
  `maestro-extensions-wasm --example prepare_author_component` and
  `maestro --bin maestro`. The tool reads the repository and commit from
  `Cargo.lock`, so a pin move installs again.

The native fixture must resolve, and so must the setup example when the workspace
has a component crate: a workspace that neither holds nor locks the package stops
preparation with a message naming it. Staging the `maestro` executable is the step
that skips: a workspace holding that package builds it instead, and one that
neither holds nor locks it stages nothing.

Two more steps run only where their input exists. The
[browser graph](#browser-author-build) is built when the workspace holds
`maestro-web`. For each search program the workspace's `mise.toml` pins as
`ripgrep` or `fd`, preparation resolves the required version from its full `PATH`
and copies it into `<TEST_PREPARE_OUTPUT>/search-tools/rg` or `search-tools/fd`,
with `.exe` on Windows. Missing or wrong-version programs fail preparation.
`mise.toml` is a bundle-cache input, so a pin change rebuilds the bundle.

The `author-component` setup exports `MAESTRO_TEST_RG` and `MAESTRO_TEST_FD` as
absolute paths to those copies. The prepared example receives these paths without
searching `PATH`; archived tests need neither program on `PATH`. A local run
without a prepared bundle still resolves the pins from `PATH`. A workspace that
pins neither program exports neither variable.

Native search tests call `maestro_test_process::install_search_program("fd", bin.path())`
(or `"rg"`) to copy the exported program into their own directory. The helper keeps
the program's permissions and returns its installed path. The caller retains the
directory guard and controls its manager, child `PATH` or child variables.
The tooling search-setup tests use the same exports to install private programs
for their preparation and local-setup children. Their runner needs neither
program on `PATH`; archived-export children also exclude the private directory.

The tool-factory suite in `maestro-tools` and the factory-construction test in
`maestro-extensions-wasmtime` install the pins the same way, so neither needs `rg` or
`fd` on `PATH`. The `author-component` filter in `.config/nextest.toml` covers each
test that receives the exports: add a test to it when the test calls
`install_search_program`.

Test children get stub `fd` and `rg` programs in their binary directory, from one
helper in `maestro-test-process`: `install_stub_search_programs`. The chat crate's
isolated children and the executable crate's `Root::command` children use it. A child
that starts the interactive mode then neither downloads a search program nor runs one
found on `PATH`. A test that supplies the child's `PATH` skips the stubs and installs
whatever programs it needs.

`SearchGuard`, from the same crate, wraps a child's `PATH`: it places a guard `fd` and
`rg` ahead of the first directory outside the child's root, unless the `PATH` names no
such directory. If the child runs one, the test fails and names the program, whether or
not the machine has `fd` or `rg` installed. The chat harness asserts it after each
isolated child. A `Root` asserts it when it is dropped.

Nextest setups read the bundle: `MAESTRO_AUTHOR_COMPONENT` and its siblings,
`MAESTRO_NATIVE_FIXTURE`, `MAESTRO_CHILD_TOOLCHAIN_PROBE`,
`MAESTRO_STARTUP_CONVERSION`, `MAESTRO_BROWSER_IMPORT_CHECK`, `MAESTRO_GUIDE_COMPONENT`
and `MAESTRO_EXECUTABLE`. Declare them in the workspace's own
`.config/nextest.toml`; Maestro's file declares the setups this repository uses.

`nextest-setup executable` exports `MAESTRO_EXECUTABLE` from the bundle for a
workspace whose tests start Maestro's executable but do not hold its package. It
serves the bundle prepared from the commit the lock pins, and refuses one installed
from another source, so a pin move cannot hand the tests the previous executable.
A workspace that holds `maestro` builds the executable with Cargo, so the setup
refuses it instead of exporting a second copy: there, tests use Cargo's own
executable path.

### Browser author build

A test shard starts with an empty target directory and no compile cache. The
browser author tests run the browser recipes, which compile the native bootstrap
programs and the WebAssembly fixture, and lint the delivered browser library.
Whichever of them ran first on a shard paid for all of that while about a thousand
other tests used the same cores. That took 120 to 290 seconds, and the test that paid
changed with scheduling, so any of them could reach the hang limit.

The test preparation pays the cost once per bundle: it builds the graph with the recipes
below in `target/browser-author` of the checkout and stages that directory in the bundle,
without `debug/incremental`, which Cargo rewrites on the next build and which is a seventh
of its size. The bundle manifest records whether it holds a graph; a build that fails leaves
it out and the shards build it themselves. A shard's `nextest-setup browser-author` replaces
`target/browser-author` with the staged graph, after checking that the checkout still holds
the files the bundle was prepared from, and gives every file of it one modification time
taken when it installs, through a read-only handle on Unix. Cargo rebuilds a unit when a
source or an output it builds on is newer than its fingerprint, so a graph that is newer
than the checkout and has no output newer than another leaves the recipes nothing to
compile.

The setup runs before the first browser author test of a run, and only on a shard that runs
one. Without a bundle it pays the whole cost itself. It lints
`maestro-web` for `wasm32-unknown-unknown` as `browser-check` does, without the
formatting repair that would rewrite sources, then runs `browser-example-check`,
`browser-library-build` and `browser-debug-build` in the checkout. It removes the
runner's coverage flags and profile path from those commands and restores the compiler
wrapper that preceded coverage, as the tests do for their own children. It builds in
`target/browser-author`, the first lane, copies the graph into `target/browser-author-1`
with one new modification time, and exports both targets as
`MAESTRO_BROWSER_TARGET_DIRS`, in the platform's path-list syntax. A copy at
another path serves the recipes as the original does: a recipe in a copied lane compiles
only what it compiles in the original. Output built for one checkout path serves another,
so each test's copy of the checkout reuses it.

The tests change sources and rebuild, and Trunk writes its binding and optimizer outputs
outside Cargo's build lock, so two tests never build in one target at once. Each test takes
a lane instead. The `browser-author` test group in `.config/nextest.toml` admits two tests
at once, and nextest numbers them from 0 in `NEXTEST_TEST_GROUP_SLOT`. `Browser` gives every
recipe and helper the lane with that number and holds a lock file inside it while the
fixture lives. The group's `max-threads` equals `LANES` in the setup's browser graph module;
`browser_author_group_admits_one_test_per_lane` fails when the two differ. Every lane can
build at once, so the lanes split the run's compile jobs: each recipe and helper gets the
caller's `CARGO_BUILD_JOBS`, or the core count without one, divided by the number of lanes,
and at least one job. Every recipe still slows while the other lane builds: on a four-job run
three lanes of one job each flagged six browser tests slow and finished the group later,
where two lanes flag none. A test outside the group, or a run without the setup, takes the
first lane and waits on its lock, so plain Cargo tests still run the browser tests one at a
time. Each lane past the first costs one copy of the graph before the first test starts.

A test that needs a stable checkout path takes its copy with `Browser::cached`. The copy keeps
the modification times of its sources and manifests, so a lane that already linted
`maestro-web` finds the library fresh and the check lints only the fixture, which takes
about 0.4 s. Only the fixture's `src/main.rs` is stamped newer than the lane, because tests
replace it and build the replacement; a newer library source or manifest would make every
check compile the library again. `browser_check_reuses_the_compiled_library_of_the_lane`
proves it.

`browser_author_bootstraps_from_empty_build_state` starts with Maestro's own crates
unbuilt on purpose, so the browser author setup is not run for it. Only Maestro's own
crates compile in it: the workspace-sources setup below provides the registry units. The
test reserves every core with `threads-required`, so other tests do not stretch its
compiler jobs past the hang limit. The test checks one cold build and its outputs.
Rebuilds after a change to an included file, a source error and its repair belong to
`browser_author_included_files_rebuild_without_source_changes` and the
`browser_library_build_rejects_` tests, one per spoiled input. A recipe that fails
during setup prints a warning and does not stop the run; the setup then announces the
first lane alone, and the tests run the same recipes in turn and report the failure.
Plain Cargo tests, and nextest runs without the setup, use the test build's own target
directory as their only lane.

### Cargo state of tooling fixtures

Cargo locks the package cache of its home while it resolves, even with
`--offline`, so two tests that start `cargo` in one home wait for each other.
Tooling fixtures therefore run Cargo in a private `CARGO_HOME` that
`compiler::fixture_command` creates beside the fixture. The recipe
tool build, every `Workspace` command, the consumer workspaces and their hook
runs, the standalone asset and wasm probe fixtures, and lock generation and
metadata queries on the browser fixture manifest use one.

The instrumented child-toolchain probe is a `maestro-tooling` example target that
preparation builds once, so no test and no test shard compiles it. Preparation
stages it, and the `native-fixture` setup validates and exports it as
`MAESTRO_CHILD_TOOLCHAIN_PROBE` alongside the native fixture; a test launches the
exported path directly.

A test shard starts from an extracted test archive, which carries test executables
and never Cargo's build records. A probe that a test compiled would therefore
recompile its dependency closure cold in every shard. Preparation compiles it
once instead, into the run's target directory with the dev profile:

```sh
cargo rustc --locked -p maestro-tooling --example child_toolchain_probe \
    --profile dev --target-dir <run target> -- -C instrument-coverage
```

Preparation always passes `-C instrument-coverage`, so the staged parent records
counters in every run, not only in a coverage run.

A fixture that resolves registry crates reads one offline source configuration.
The nextest setup `nextest-setup fixture-sources` vendors the crates
named by `fixtures/registry_sources/Cargo.toml`, at the versions the workspace
lock selects, together with the browser fixture's crates. It runs once per run,
writes `target/fixture-sources/<hash>/`, and reuses that directory while the
inputs stay unchanged. The set stays small because Cargo reads every vendored
package on each run; a test checks that every registry crate the tooling crates
declare is named in that manifest. Keep an anonymous import for each declared
crate in `fixtures/registry_sources/src/lib.rs`. The dependency scan checks this
standalone fixture too. The setup exports the configuration path as
`MAESTRO_TEST_CARGO_SOURCES`; outside nextest the first test that needs it runs
the same command. A fixture opts in with `Workspace::use_prepared_sources` or
`Browser::cargo`, which copy the configuration into its private home. A fixture
without registry dependencies copies nothing.

The Unix-only cold browser test uses `nextest-setup fixture-sources workspace`,
which prepares the whole workspace's locked dependencies before the test. Its
source-preparation helper is also Unix-only; Windows compiles the shared fixture
helpers without the browser tests. The setup exports
`MAESTRO_TEST_WORKSPACE_CARGO_SOURCES` from `target/workspace-sources/<hash>/`.
This uses the same vendor operation with a wider dependency set, including Git
sources. The key covers the workspace and member manifests, the browser manifest, both
lockfiles and `rust-toolchain.toml`. Dependency or toolchain changes select a new
directory; Rust-only edits reuse it because workspace sources are read from the
checkout. Outside nextest, the first cold-source caller runs the same setup. The browser
shared-home isolation test prepares both source sets in its parent and forwards their
configuration paths to the child before making the shared Cargo home unusable. The cold
test copies only the source configuration into its initially empty Cargo home.

The setup also runs for the test that proves source preparation works without nextest's
exports: that test clears the exported paths and reruns the isolation test as a child, so
the child prepares the sources itself. That preparation vendored every package and built
the seed from nothing, which spent most of the test's time. The setup runs before the
tests, so the child's repeated preparation finds the directory for the same key and
returns it at once. The setup still pays for the vendoring, once per run.

The same setup also builds the cold bootstrap's native targets once per key, with the
commands the recipes run, in `target/workspace-sources/<hash>/seed-target`. It keeps the
units of registry crates and deletes the units and outputs of workspace members, the
examples and `debug/incremental`. A member's binary has a name no package spells, so its
output in `deps` goes with the fingerprint that shares its hash. The setup then
publishes the result as `seed` beside the configuration. A failed build leaves no seed
and the next run continues it. The cold test copies the seed into its empty Cargo target
directory and checks that it holds compiled units but none of a workspace member's. The
recipes then compile Maestro's crates and the WebAssembly fixture from nothing, and
every registry crate the seed holds is already built. The test keeps the root manifest's
per-package optimization levels, since the seed holds the optimized parsers. The test
never pays for the seed.

Cargo rebuilds a unit when a dependency's outputs are newer than its own, so every copy of
the seed keeps each file's modification time: the copy that stages it in the bundle, the one
that installs it beside the sources and the test's own. A copy that stamps files in name
order makes dependencies look newer than their dependents, and the cold test then compiles
registry crates the seed already holds. On a four-job run that took 59 s and 53 registry
packages, against 25 s and 12 with the times kept. The test records the seed copy's time,
the library build's time and the registry packages and workspace members the build
compiled, in its transcript and on its standard error. The 12 registry packages that remain
are the fixture's own `wasm-bindgen` packages, which the seed does not hold. A count in the
dozens means Cargo rejected the seed.

The test preparation builds the seed once per bundle. It fetches every locked package so
that the offline vendoring finds them all, prepares the workspace sources and the seed in
the checkout as the setup does, and stages the seed in the bundle as `workspace-sources`.
The manifest records the directory of the sources the seed was compiled against, because a
registry unit's fingerprint includes its source path and a seed serves only those sources.
A shard's setup vendors the sources, then copies the bundle's seed beside them when their
directory is the recorded one. The directories match when the checkout path and the
dependency key do, and then no registry crate compiles before the shard's first test.
When a member manifest that the cache key does not name changes, a bundle restored from
the cache can hold a seed for another key. The setup then builds the seed itself, as a run
without a bundle does, once per key.

The cold recipe uses `fixture_command` to keep its private home out of the
compiler wrapper's cache keys. It also leaves `CARGO_BUILD_JOBS` to the caller: a cap set
by the build host applies, and with none Cargo compiles with every core the test reserves.
The warm recipes and the native helper split the same cap between the lanes, as
[Browser author build](#browser-author-build) describes.

Cargo builds that share one target directory with other builds, the test build's or a
lane the browser author setup prepares, keep the shared home.
A registry crate's fingerprint includes its source path, so a private home with
vendored sources recompiles every registry dependency in that target, and going
back to the shared home recompiles them again. The browser recipe and native
builds (`Browser::command` and `Browser::native_command`) are such builds; each runs alone
in the lane its test holds.
Queries of the whole workspace, tests in other crates that build it, and the
in-process `maestro_tooling::run` calls on dependency-free fixtures also keep the
shared home. A watch recipe starts `cargo run` and, beneath it, `cargo build` in one
home, so its first build prints a wait on its own `cargo run`, not on another
test.

On Unix, when `RUSTC_WRAPPER` or, failing that, `CARGO_BUILD_RUSTC_WRAPPER` is set,
`fixture_command` points `RUSTC_WRAPPER` at the prepared fixture's `hide-home`
wrapper, installed as `rustc-wrapper` in the fixture root. Shared CI sets the second
variable. The wrapper unsets `CARGO_HOME` and `CARGO_TARGET_DIR` and runs the
original wrapper, which it reads from `MAESTRO_FIXTURE_RUSTC_WRAPPER`. `sccache`
hashes every `CARGO_*` variable into the key of each rustc call, so a wrapper that
saw the private home or the per-run target directory would give the registry crates
of each fixture a key that no other fixture shares. Cargo itself keeps both
variables and passes the compiler its output directory explicitly.
`fixture_command_hides_private_cargo_paths_from_the_compiler_wrapper` runs a
fixture build behind the `recording` wrapper, which logs the `CARGO_HOME` and the
`CARGO_TARGET_DIR` its compiler saw, and checks that it never sees either.
`fixture_command_wraps_an_inherited_cargo_build_rustc_wrapper` does the same for a
wrapper inherited as `CARGO_BUILD_RUSTC_WRAPPER`.

Three tests (`workspace_fixtures_never_reach_the_shared_cargo_home`,
`browser_resolution_never_reaches_the_shared_cargo_home` and
`consumer_workspace_never_reaches_the_shared_cargo_home`) rerun a recipe
workspace, a browser resolution and a consumer workspace in a child whose shared
`CARGO_HOME` holds a directory where Cargo expects its lock file. A fixture that
still reaches the shared home fails at once instead of waiting for another test.

### Prepared Cargo checks

Run the browser import checks with their setup:

```sh
just prepare-tests
cargo nextest run --locked -p maestro-models -E 'test(/^browser_build::/)'
```

The `cargo-checks` setup builds `browser_import_check` for
`wasm32-unknown-unknown` with the locked development profile. It also creates a
clean, independent author project from the extension guide's manifest fixture,
its lock, the shipped greeting source and the pinned toolchain, then builds its
library for `wasm32-wasip2` with the locked release profile. Both builds use
Cargo's reported artifact paths and remove host coverage instrumentation through
the browser setup's command policy. A failed build reports the compiler's
diagnostics.

Preparation stages those outputs in the existing `target/test-setup` bundle and
records their digests in `cargo-checks.json`. Its source fingerprint covers the
`maestro-models` and `maestro-extensions-wasm` package closures, the workspace
manifest, Cargo configuration, toolchain and lock, the guide fixture directory
and `examples/extensions/hello.rs`. Edits outside those inputs, including tooling
source changes, leave these outputs current.
Local nextest setup reuses current outputs or builds them before any test body.
Archived runs validate both outputs before exporting their relocated absolute
paths as `MAESTRO_BROWSER_IMPORT_CHECK` and `MAESTRO_GUIDE_COMPONENT`. Missing,
edited or stale archived outputs fail setup without invoking Cargo.

The browser test reads the real build's WebAssembly artifact. The guide test
checks its documented manifest, discovers the independently built extension in a
clean runtime directory and invokes its greeting tool. Neither test body starts
a compiler. Re-run preparation when the bundle is stale; do not add a fallback
build to a test body.

### Child-process tests

Stage the setup tool before a filtered native-fixture run:

```sh
just prepare-tests
cargo nextest run --locked -p maestro-test-process -p maestro-process
```

Use checked reruns when the test owns the invocation and harness framing. Use the
prepared `native_fixture` example when a backend launches a program by name with
literal operands, or when its protocol requires raw bytes. The complete contract
is [Rust test children](specs/test-processes.md).

The package re-pin matrix shares one native Git history per repository layout.
Before each reference form, it restores the original HEAD selection, revision and
in-memory settings, then clears dependency observations. The matrix covers failed
acquisition, successful retries, missing refs and unchanged revisions without
recreating or republishing the history for each form.

#### Direct native fixtures

Nextest's `native-fixture` setup exports `MAESTRO_NATIVE_FIXTURE` to
`maestro-test-process` and every crate the crate graph allows to depend on it, and
the instrumented parent `MAESTRO_CHILD_TOOLCHAIN_PROBE` to `maestro-tooling`.
`maestro-app` also reads the startup conversion child, `MAESTRO_STARTUP_CONVERSION`.
That child is the `startup_conversion` example of `maestro-app`: it runs one conversion, `auth`
or `sessions`, as its first argument names, and prints one `PROBE` line with the outcome.
`--file-limit-kib N` runs the conversion under a file-size limit that is lifted before it
reports, and `--exit-write PATH` then writes 4096 bytes to `PATH`. A test starts it as a plain
command: it needs no libtest arguments and starts in milliseconds where the whole
`maestro-app` test program takes tens of seconds. The test sets `MAESTRO_CODING_AGENT_DIR` and its working
directory, and passes `IsolatedSetup::environment` for effects such as continuing past the
file-size limit. Preparation builds the child once, like the other fixtures. It keeps its own
record, keyed on the crates `maestro-app` builds on, so an edit to one of them rebuilds the
child alone and an edit to the native fixture's crates leaves it staged.
To declare a new caller in the [crate graph](specs/maestro-port.md#test-process-owner),
add it to `TEST_PROCESS_CALLERS` and add its own `package(=name)` line to
`.config/nextest.toml`. Adding fixture tests inside a declared caller needs no
shared selection or docs edit. The conventions check rejects missing or extra
crates and requires every filter term to be exactly `package(=name)`, joined by `|`.
Run `just prepare-tests` before a filtered nextest run on any declared caller.
Local setup builds the example with the locked host `dev` profile and reads Cargo's
emitted executable path. It records source inputs, content digest and host in
`target/test-setup/native-fixture.json`; unchanged inputs reuse that artifact.
Prepared archive runs validate the staged copy and export its remapped absolute
path without building. A missing or stale archive artifact fails setup. Re-run
preparation instead of adding a target-directory search or a runtime compiler.

Call `install_executable(artifact, destination)` with that explicit path. The
caller's disposable directory owns the alias. Installation runs inside the same
artifact and waits for the copy's writer to close; the test runner never opens a
writable executable that a sibling child could inherit. Installing over an
existing alias replaces its file instead of rewriting it, so an alias that
already ran is never changed in place. Copies have independent
modification times. A new copy keeps the artifact's time on macOS and Windows
and takes the copy time on Linux, so a test that compares an alias's time with
other files sets that time itself. Windows aliases use the native `.exe` suffix.

The observation scenario reads `<alias>.fixture`, including the `.exe` suffix
when present. The record has exactly three lines: `observe`, the parent's
nonzero TCP port and an exit status from 0 to 255. It reports its actual PID, cwd,
`MTP_VALUE` and each unmodified operand on that socket as length-prefixed native
strings (eight-byte little-endian byte length). It closes the socket, writes
`err` followed by NUL, byte FE and `!` on stderr, echoes stdin unchanged to stdout
and exits with the supplied status. Neither output stream carries harness text.
An unknown or malformed record fails before connecting or writing the optional
`MTP_EFFECT` sentinel. Direct invocation uses `--maestro-observe <record> <operands>`;
installed aliases leave all operands untouched. Add cohesive finite Rust fixture
families to this one target, never a script evaluator or another launcher.

The portable `editor` family reads one JSON object after the family line. To
report the edited path, set `result` to the name of the environment variable
holding the output path. It skips its own `--custom` option and writes the next
operand followed by a newline. A missing variable fails before file effects.
For example, with `MAESTRO_EDITOR_RESULT=/scratch/editor-result`, this
`vi.fixture` record makes `vi --custom notes.txt` write `notes.txt` followed by a
newline to `/scratch/editor-result` and exit 7:

```text
editor
{"result":"MAESTRO_EDITOR_RESULT","code":7}
```

The process tests install the alias under the name the product resolves from
`PATH`, so shell evaluation stays real. The same record can select the file
effects described under [Viewer, editor and trash aliases](#viewer-editor-and-trash-aliases).
The `echo` family prints the single line following its family line, so executable
selection tests can run a native program that prints their authored name.
Neither family writes harness text.

#### Command stand-ins

The `command` family replaces an interpreted stand-in for a program the
product launches by name. Every scenario works on every platform except `labelled`,
`stdio`, `hold`, `forward`, `hold-hangup` and `await-release`, and the `SIGTERM`,
`SIGKILL` and `SIGRTMAX` signals of `output`. Those scenarios, `SIGTERM` and
`SIGKILL` need Unix; `SIGRTMAX` needs Linux. Install the artifact under the name the
product uses, then write `command` on the first line of `<alias>.fixture` followed by a JSON
object whose `scenario` selects one finite behaviour. The fixture parses the whole
record before any effect and rejects an unknown scenario or field, including in the
`labelled`, `stdio` and `package` scenarios that declare no fields of their own.

| Scenario | Fields and behavior |
| --- | --- |
| `output` | `stdout` and `stderr` are byte arrays and `code` is the exit status. Optional `signal` ends by that native signal instead: `"SIGTERM"`, `"SIGKILL"`, or `"SIGRTMAX"` on Linux for the real-time signal that carries no shared name. Optional `split` writes each standard output byte on its own, so a reader must reassemble split UTF-8. Optional `append` (`path`, `bytes`, `cwd` to append the working directory first, and `variable` to append that variable's `name=<text>` line, empty when unset) appends to that file. Optional `argv_log` (`path`, `mark`) appends `mark` and each native operand, separated by NUL. |
| `labelled` | Reports the first operand on standard output, `<operand>-err` on standard error, then one line of input as `stdin:<line>`. It reads exactly one line, so the next sibling still receives the rest. |
| `report` | `fields` lists what to print, joined by one bar in that order: `{"variable": "NAME"}` for a variable that reads empty when unset, `"working-directory"`, `{"operand": N}` for one operand by position, and `"input"` for the number of bytes standard input held. |
| `stdio` | Prints every operand in angle brackets, `out-<invoked name>`, one line of input as `stdin:<line>` and the working directory on standard output, and `err-<invoked name>` on standard error. |
| `npm` | Answers package queries: `view` prints the optional `view` field, `root` prints the optional `root` field, and `install` requires `install.argument` as its second operand before writing `install.path` with `install.contents`. Optional `log` records each invocation like `argv_log`. An unrecorded query or any other command exits 1. |
| `hold` | Writes `"ready"` to optional `ready`, reads one byte from the `permit` FIFO, then writes `stdout`, `stderr` and optional `append`. Optional `late` names a FIFO read before the permit, for a writer that opened it first. Optional `terminate` appends and exits 0 when SIGTERM arrives, so only a termination signal ends the process after the gate. It drains standard input first, which ends when the process that started it exits. |
| `holder` | Starts the program named by `descendant` with a shared input pipe, writes `stdout` and `stderr`, and exits at once. The descendant receives end-of-input when the holder ends, so it runs after it, and it keeps the inherited streams open until it finishes. |
| `query` | Requires a value followed by `root -g`, prints `prefix` and the value with its newline, or writes `error` and exits `code` for the `denied` value. |
| `package` | Appends the working directory, the invoked name and every operand, tab-separated, to `MAESTRO_COMMAND_LOG`. Requires `--quiet` first, answers `root` with `MAESTRO_NPM_ROOT`, `view` with `"2.0.0"`, admits `install`, and fails anything else. |
| `hold-hangup` | Requires one socket path. Registers the hangup, prints `ready` on a line of its own, and when the hangup arrives writes `h` to the socket and runs on until terminated. |
| `await-release` | Requires one socket path. Writes `w` to it, then ends once it receives one byte. |
| `report-environment` | Prints `seen:<ROLE>\|<MAESTRO_PANE_ID>\|<working directory>` on a line of its own, then runs on until terminated. |
| `metadata` | Requires `metadata` first and fails anything else before any effect. Reads and prints the file named by `declared` when an operand is `--no-deps`, and the file named by `resolved` otherwise. The files are read on each run, so a test rewrites them between runs. Optional `argv_log` (`path`, `mark`) appends the invocation as `output` does. |
| `forward` | Appends `marker`, then replaces itself with `program`, which receives the untouched operands. |
| `prefixed` | Requires `--prefix` and a prefix operand first. For `root`, writes the prefix followed by `suffix` and a newline. Every other invocation appends its operands to `log`, one per line. |

Use `install_command(alias, record)` to copy the artifact and write its sibling
`.fixture` record, then configure the alias as the command the product launches:

```rust,no_run
fn main() -> std::io::Result<()> {
    let directory = maestro_test_directory::Scratch::new()?;
    let alias = directory.path().join("npm");
    maestro_test_process::install_command(
        &alias,
        "{\"scenario\":\"npm\",\"view\":\"\\\"2\\\"\",\"root\":\"/package/root\"}",
    )?;
    let _settings = serde_json::json!({"npmCommand": [alias.to_string_lossy()]});
    Ok(())
}
```

Keep each alias in a test's disposable directory and pass its absolute path as the
configured command. The stand-in inherits the child's environment, so the two
`MAESTRO_*` variables above belong to the process that launches it. A `hold`
record may omit `ready` when no caller reads that marker. A `holder` names its
descendant by an installed alias, whose own record gives that descendant its work.

Another family takes the same route: `install_family(alias, family, record)` copies
the artifact and writes the family and its record, and `install_command`, the
credential, Git and peer installers all call it. A test that installs a record of
its own, such as an editor or a viewer, calls it directly instead of writing the
`.fixture` file itself.

A `hold` child waits on FIFOs that Rust's standard library cannot create.
`gate::Gate::new(path)` uses `/usr/bin/mkfifo` on Linux and macOS to create the
FIFO with owner-only read/write permissions, then keeps it open for reading and
writing until the guard drops. The absolute executable path also works when a
test replaces `PATH` with its fixtures. A failed creation returns an error
without opening or removing an existing file. Pass `gate.path()` as `permit` and
`gate.release()` lets one waiting child continue. Pass a second gate as `ready`
and `gate.take(5)` waits for a child's `"ready"` bytes, so a test holds one child,
observes it started, releases it and then observes the next, without a sleep or a
poll. Dropping the guard removes the FIFO: a waiting child reads the end of the
stream and ends, and a child that starts later finds no FIFO, so a failed test
never leaves one blocked. The gate exists on Unix only.

The conventions checker's architecture tests answer its Cargo metadata and compiler
queries the same way. `Workspace::metadata_command` installs a `metadata` Cargo whose
`declared` and `resolved` files sit in the fixture workspace, and `Workspace::command`
installs an `output` compiler that prints a host line, fails, or prints text without
one. Both probes run in a child, and a failing alias stands in for a compiler wrapper
that must never run. A test that holds a writer open does so on the alias's `.fixture`
record, because a writer on the executable itself makes the launch fail with
`Text file busy`.

The daemon's layout-apply tests launch these as pane commands. The test owns a Unix
socket and a thread that answers `w` and `h` records: a `hold-hangup` leaf keeps its
pane alive through the hangup, and an `await-release` leaf ends only after that
hangup was reported, so the ordering needs no file polling.

A command that only prints installation output uses the `output` scenario. The
command-line startup tests install it as `controlled-npm` and write its two install
lines, so the protocol modes keep an empty standard output while the chatter reaches
standard error. A cleared child needs `MAESTRO_NATIVE_FIXTURE` passed on, because
`env_clear` drops the preparation variable.

The preparation tests use a `tooling-cargo` record with the real Cargo path and
an author-record template path. It forwards metadata, accepts the Clippy probe
and stages native copies of both preparation examples. It also reports the two
finite WebAssembly artifacts used by the Cargo-check preparation fixtures. For the
workspace sources it records each fetch, vendor run and seed build (a build that passes
`--config`) in a `sources` file in its working directory. A vendor run creates its
directory and prints the source configuration; a seed build leaves the same units as the
`tooling-sources` `sources` record.
On Unix, metadata replaces the fixture process, keeping its native PID and signal
delivery.

#### Interactive-mode stand-ins

On Unix, the `chat` family stands in for the programs the interactive mode launches:
its shell, multiplexer, package manager, Git and GitHub command line. The external
editor is the `editor` family, the clipboard reader is the `clipboard` family's
`paste` scenario and a search probe the interactive mode must see in flight is the
`search` family's `probe-wait`. Install the artifact under the name the mode
resolves from `PATH`, then write `chat` on the first line of `<alias>.fixture`
followed by a JSON object whose `scenario` selects one finite behavior. The fixture
parses the whole record before any effect and rejects an unknown scenario, step,
frame or field. The interactive-mode tests install each alias in their disposable
root and keep `MAESTRO_NATIVE_FIXTURE` in the cleared child's environment, because
`env_clear` drops it.

| Scenario | Fields and behavior |
| --- | --- |
| `shell` | `steps` run in order for each `<shell> -c <command>`: `{"print": text}`, `{"print-command": {"before", "after"}}` (the command text between two texts), `{"mark": target}` (create an empty file), `{"hold": target}`, `{"log": path}` (append one empty line), `"block"` (wait for a signal) and `{"exit": status}`. A target is `{"path": p}` or `{"command": suffix}`, the command text followed by the suffix. |
| `tmux` | Answers `show -gv extended-keys` and `show -gv extended-keys-format` with the `extended_keys` and `extended_keys_format` replies (`text`, optional `code`, optional `signal`, `"SIGTERM"`, to end by that signal after writing). |
| `gated-tmux`, `gated-package`, `gated-probe` | Require the operands of the query, report the launch (`argv`, `cwd`, and `MAESTRO_NOTICE_INHERITED`) as JSON in `<directory>/<name>.ready`, wait for the line `go` in `<name>.permit`, then answer. The tmux query ignores SIGTERM. |
| `package-version` | Appends the working directory and each operand, joined by `\|`, to `log` and prints `"2"`. |
| `package-remote` | Appends one JSON line (`cwd`, `argv`, `prompt`) to `log` and answers `rev-parse HEAD`, `rev-parse --abbrev-ref @{upstream}` and `ls-remote origin refs/heads/main`. |
| `gh` | `state` receives `argv.jsonl`, one JSON array per call, and the copied document. `auth status` follows `auth` (`exit`, `remove_self`, `signal`). `gist create` plays `gist.frames` against its last operand: `{"stdout": bytes}`, `{"stderr": bytes}`, `{"send": bytes}` and `"hold"` on the control connection at `gist.port`, `"release"`, `{"chmod": octal}` and `{"copy": name}`, then ends with `gist.exit`. |
| `pause` | Waits until a signal ends the process. |
| `group-member` | Prints `ready` once it handles the stop signal, then prints `TSTP` if the stop arrives, or ends silently when a byte of input arrives first. |

A held shell waits on a gate the test owns. Bind a Unix listener at the target path
before the command starts; the shell connects and waits. Dropping the listener
releases it, whether the shell connected before or after. A `release` frame asks the
test for one grant on the control connection by sending `R` and waits for any reply.

```rust,no_run
fn main() -> std::io::Result<()> {
    let artifact = std::env::var_os("MAESTRO_NATIVE_FIXTURE")
        .ok_or_else(|| std::io::Error::other("run preparation with nextest first"))?;
    let directory = maestro_test_directory::Scratch::new()?;
    let shell = directory.path().join("shell");
    maestro_test_process::install_executable(std::path::Path::new(&artifact), &shell)?;
    let gate = std::os::unix::net::UnixListener::bind(directory.path().join("job-release"))?;
    let steps = r#"[{"mark": {"command": "-started"}}, {"hold": {"command": "-release"}},
        {"print-command": {"before": "", "after": " output\n"}}]"#;
    let mut record = shell.clone().into_os_string();
    record.push(".fixture");
    std::fs::write(record, format!("chat\n{{\"scenario\":\"shell\",\"steps\":{steps}}}"))?;
    // Configure the shell path, run `!job`, wait for `job-started`, then release it.
    drop(gate);
    Ok(())
}
```

A search probe that only answers, a quiet browser opener and a hook that only refuses
use the `command` family's `output` scenario instead.

#### Component execution stand-ins

The `component-exec` family covers the two guest-component programs no other family
can play: one that creates a file and one that reruns a test binary. The component
passes the alias path and its literal operands through the same request it always
used. `touch` works on every platform and `rerun` needs Unix. A child that holds a
connection, answers termination or splits output across pipe writes uses the `exec`
family. A program that only writes both streams and ends uses the `command` family's
`output` scenario. One that reports its working directory and operands uses the
`child` family's `arguments` scenario.

| Scenario | Fields and behavior |
| --- | --- |
| `touch` | Creates the file its only operand names. |
| `rerun` | Replaces the process with `binary` running `test --exact --nocapture`. Unix only. |

The guest example registers a shell with `exec shell:{"program": path, "operands": [...]}`:
each command then runs as the last operand of that program, which the `exec` family's
`holder` ignores.

```rust,no_run
fn main() -> std::io::Result<()> {
    let artifact = std::env::var_os("MAESTRO_NATIVE_FIXTURE")
        .ok_or_else(|| std::io::Error::other("run preparation with nextest first"))?;
    let directory = maestro_test_directory::Scratch::new()?;
    let program = directory.path().join("arguments");
    maestro_test_process::install_executable(std::path::Path::new(&artifact), &program)?;
    let mut record = program.clone().into_os_string();
    record.push(".fixture");
    std::fs::write(record, "child\n{\"scenario\":\"arguments\"}")?;
    // Execute `program` with literal operands; it prints its directory and them back as JSON.
    Ok(())
}
```

A timing assertion keeps its child alive until the deadline ends it, by holding the
connection open; it never relies on how quickly the program starts.

#### Repository recipe stand-ins

A recipe workspace answers its own commands without an interpreter. Install the
prepared artifact under each name the recipe resolves from `PATH`, then run the
recipe:

```rust,no_run
fn main() -> std::io::Result<()> {
    let artifact = std::env::var_os("MAESTRO_NATIVE_FIXTURE")
        .ok_or_else(|| std::io::Error::other("run preparation with nextest first"))?;
    let directory = maestro_test_directory::Scratch::new()?;
    let bin = directory.path().join("bin");
    std::fs::create_dir_all(&bin)?;
    for name in ["cargo", "mise", "prek", "watchexec", "trunk", "miniserve"] {
        let alias = bin.join(name);
        maestro_test_process::install_executable(std::path::Path::new(&artifact), &alias)?;
        let mut record = alias.clone().into_os_string();
        record.push(".fixture");
        std::fs::write(record, "tooling-command\n{\"scenario\":\"stand-in\"}")?;
    }
    Ok(())
}
```

The record's first line selects the family and its `scenario` selects one finite
behavior; an unknown family, scenario or field fails before any effect.

| Scenario | Behavior |
| --- | --- |
| `tooling-command` `stand-in` | Prints `controlled command stdout` and `controlled command stderr` when `MAESTRO_COMMAND_OUTPUT` is set. Runs the real Cargo in `MAESTRO_REAL_CARGO`, and after a successful `build` runs the staged `target/debug/maestro-models` and `target/debug/development --rebuild-marker`. Forwards `run -- <program>` to `MAESTRO_DEVELOPMENT` and a `watchexec` invocation to `MAESTRO_REAL_WATCHEXEC` with `--wrap-process=none`. Otherwise it appends the invoked name, a space and the operands joined by a vertical bar to `MAESTRO_LOG`, writes the working directory to `MAESTRO_CWD`, writes the brand and environment observations to `MAESTRO_BRAND_RESULT` and `MAESTRO_ENV_RESULT`, and ends with status 17 when `MAESTRO_FAIL` names the first operand or `MAESTRO_FAIL_ARG` names any operand. |
| `tooling-command` `forward` | Appends the operands, separated by spaces, to `MAESTRO_NATIVE_LOG`. Writes `controlled native tool failure` and ends with status 17 when `MAESTRO_NATIVE_FAIL` is `1`, except for a `--version` first operand. Otherwise runs `MAESTRO_NATIVE_TOOL` with the same operands. Browser-author fixtures install it under `wasm-opt`, so the recipe still calls the real optimizer and the test reads what it received. |
| `tooling-sources` `sources` | Records `cargo vendor` calls in the record's `calls` file and `cargo build` calls in `builds`; writes `vendor exploded` or `build exploded` and ends with status 1 when `fails` names that operation. Otherwise a build leaves the seed units in `CARGO_TARGET_DIR/debug`, and a vendor creates the last operand as a directory and prints the vendored-source configuration. The fixture-sources setup installs it as `cargo`. |
| `tooling-sources` `operands` | Prints each operand on its own line, writes the record's `stderr` text, and ends with the record's `code`. Tests use it to prove an adapter passes literal operands, diagnostics and the status through unchanged. |
| `tooling-sources` `trunk-hook` | Copies `assets/app.css` to `hook.css`, writes the first operand and the `TRUNK_*` and `MAESTRO_HOOK_INHERITED` variables, one per line, to `hook-environment`, and ends with the status in `MAESTRO_HOOK_STATUS`. Installed as an authored build hook. |
| `tooling-sources` `call-log` | Appends `call` and the operands to the record's `path`, followed by every variable whose name starts with one of the record's `prefixes`, sorted. Ends with status 1 when the first operand is the record's `refuse`. |
| `tooling-sources` `artifacts` | Appends the operands to the file `CARGO_CALLS` names, then reports one compiled artifact per name in the record, at `/built/<name>.wasm`. |
| `tooling-sources` `consumer-cargo` | Appends the operands to `cargo-log`, beside the directory holding the alias. Ends with status 17 when `MAESTRO_FAIL_PHASE` names the first operand, succeeds without work for `hakari`, `test` and `nextest`, and runs the record's `cargo` for everything else. |
| `tooling-sources` `announce` | Prints `cargo` and the operands. When the record names `rebuilt`, it also replaces that path with a copy of itself that prints `example`. |
| `tooling-wrapper` `hide-home` | Runs the compiler wrapper named by `MAESTRO_FIXTURE_RUSTC_WRAPPER` with `CARGO_HOME` and `CARGO_TARGET_DIR` removed. The child-toolchain builder installs it as `rustc-wrapper` in every fixture root, so the compiler cache never keys a fixture's dependencies on that fixture's private Cargo home or output directory. |
| `tooling-wrapper` `recording` | Appends the `CARGO_HOME` the compiler saw, or `unset`, to a log beside the alias, and the `CARGO_TARGET_DIR` it saw, or `unset`, to a second log with the `.target.log` suffix, then runs the first operand with the remaining ones. A test installs it as an inherited `RUSTC_WRAPPER` or `CARGO_BUILD_RUSTC_WRAPPER` to observe the same boundary. |

A runner that starts `just build` in a recipe workspace therefore never needs
Cargo, `mise` or `watchexec` of its own: each call reaches the installed alias,
and only the operations that need the real program forward to one. Installing
needs the artifact nextest's `native-fixture` setup exports, so run
`just prepare-tests` first; without it, a fixture command that needs the wrapper
fails instead of building a fallback.

The author example reads a `tooling-author` line followed by one JSON scenario.
For example:

```text
tooling-author
{"scenario":"cached","engine":"fixture"}
```

`cached` exports supplied component paths and writes compiled-code observations;
`engine` is `fixture` or `newer`. `plain` exports without writing compiled code.
`malformed` writes an invalid environment entry, and `unstable` changes a source
after export. `misplaced` writes a cache namespace with the wrong executable time;
`split` writes two namespaces. `marked` builds only unsupplied components and
writes their dependency files. Unknown scenarios, engines and fields fail before
preparation effects.

`prepare_author_component --cache-directory` reports one absolute JSON path and
creates no environment file, component, cache entry or observation log. The fixture
reports `component-cache` below an absolute `MAESTRO_CODING_AGENT_DIR` and fails the query
when the variable is missing or relative. The component setup always supplies an absolute
agent directory, so the failure is reachable only when the fixture runs outside that
setup. The built alias
has a sibling record. When a test exercises product copying or relocation,
set `MAESTRO_NATIVE_RECORD` on the child command to select the author record
explicitly; the product does not copy fixture sidecars. Only the author-example
alias reads this selector, so nested Cargo and other aliases retain their own
records. Other tooling fixture families keep their existing contracts.

#### Retained native boundaries

These paths keep a real interpreter or native program because that program is the
product operation under test. A controlled stand-in would remove the behaviour the
test observes.

| Path | Retained operation | Reason |
| --- | --- | --- |
| `crates/maestro-credentials/src/credentials/tests.rs:179`, `:219` | `/bin/sh` and `/system/bin/sh` compared as launch descriptions. | The configured and default shell is the product behaviour, and these values never start a process. |
| `crates/maestro-credentials/src/credentials/tests.rs:205-215` | One real `!if read line; ...` resolution through that shell. | The resolver's configured shell is the operation under test. |
| `crates/maestro-credentials/src/credentials/tests.rs:240-266` | Windows `cmd.exe` and `bash.exe` launch descriptions. | The launch shape is the product behaviour under test, and these values are compared, never run. |
| `crates/maestro-credentials/tests/credential_key_selection.rs:136-149` | The Windows Git Bash, `bash.exe -c`, that runs configured helpers. | The helper launch needs the native shell the selector finds. |
| `crates/maestro-credentials/tests/credential_key_selection.rs:153-163` | `!printf` payloads, one piped through `tr`, resolved by that shell. | Every payload is authored shell text, and the assertions are on the resolver's result. |
| `crates/maestro-credentials/tests/credential_key_selection.rs:391-396` | `!exit 1`, a missing helper, empty output and whitespace. | The same resolver, and the assertions are on its result domain. |
| `crates/maestro-credentials/tests/config_values.rs:407-554` | Real `!printf`, `sed`, `cat` and `exec` payloads through the resolver. | The resolver's shell is the subject: pipes, multiline output, invalid UTF-8, inherited environment, cwd and the deadline all come from a real child. |
| `crates/maestro-extensions/tests/support/action_checks.rs:1069`, `:1073-1074` | `/bin/sh -c` as the program handed to the extension `exec` operation. | `exec` is the subject: its captured streams, exit code and abort must come from a real child. |
| `crates/maestro-daemon/src/panes/lifecycle/tests.rs:388`, `:745`, `:791` | `/bin/sh -c` pane scripts with `trap`, background jobs and `exec`. | The pane's shell and its process-group teardown are the subject: members that ignore hangup must still be reaped after the shell. |
| `crates/maestro-daemon/tests/pane_input.rs:198` | A line typed into the live pane shell. | The pane terminal is the subject: its input reaches a real shell. |
| `crates/maestro-daemon/tests/layout_apply.rs` (`/bin/sleep 60` leaves) | The system `sleep` program as a long-lived pane command. | The system `sleep` is a native program, and the leaf only has to outlive the test body. |
| `crates/maestro-rpc/tests/daemon_socket.rs:745-750` | The daemon's startup pane running the configured `/bin/sh`. | The pane terminal is the subject: one case writes to the live shell and reads its answer back through the protocol. |
| `crates/maestro-rpc/tests/support/served.rs:281` | The native `kill` program signalling a live protocol child. | Real signal delivery to the observed child is what the case qualifies. |
| `crates/maestro-rpc/tests/transports.rs:330` | The native `kill` program signalling the test process itself. | Real handler registration and ordering are the subject. |

#### Application RPC peers

Run the client corpus with its prepared native peers:

```sh
just prepare-tests
cargo nextest run --locked -p maestro-rpc -E 'test(/rpc_client/)'
```

On Unix, `maestro_test_process::rpc_peer::Scenario` selects a compiled peer
behavior. Call `maestro_test_process::rpc_peer::install(entry, scenario)` to copy
the prepared artifact and write its sibling `<alias>.fixture` record, then launch
the alias through the existing `RpcClient`. The record contains `rpc-peer` and
`scenario.name()` on separate lines. For example, `rpc-peer\nServe\n` captures
each raw request line before answering it. `PEER_ANSWERS` supplies an object keyed
by command type. A missing key omits response data; a JSON null stays null. The application tests' `Peer::options`
writes this record and passes the release socket and descendant alias paths.

`PEER_RECORDS` supplies arrays for event bursts or unmatched JSON values;
`PEER_REPLIES` supplies full response objects for replay. Launch observations use
`PEER_MARKER`. Raw fragments, ticks and descendant-held pipes use `PEER_PORT` for
acknowledged writes or release, without fixture sleeps. The descendant announces
readiness before its parent exits and retains the actual inherited descriptors
until released. Startup failures, closed input and termination resistance remain
native process observations. Unknown scenarios, malformed records and invalid
input shapes fail before protocol output. These peers emit no test-harness text.

The interactive client example uses `PEER_STATE` for its session state. Its idle
interrupt peer reads commands on one thread and records TERM through a native
signal flag. On EOF, it reports `terminated` only if TERM was observed, followed
by `input closed` over the release socket. Plain EOF exits without waiting for a
signal. To check both paths, send TERM before closing input, or close input alone.

The terminal client example and the client example's checks use the same family on
every native platform. `DialogTour` answers a prompt, four correlated dialogs and
six notices before ending with status seven; `LocalDialogTour` answers the local
demonstration prompt; `Notifications` reports each notice severity. The remaining
scenarios keep authored text literal:

| Scenario | Behavior |
| --- | --- |
| `SelectionReplies` | Emits a padded and an empty selection and requires each value in its own correlated reply. |
| `DialogText` | Reports dialog chrome that contains terminal controls and requires the answer to repeat them. |
| `MultilineCommand` | Requires a confirmation of a message that holds its own blank line. |
| `LiteralText` | Refuses the prompt, then streams text, a tool name and a control sequence as data before ending normally. |
| `TerminationMarker` (Unix only) | Reads the first termination signal and writes `SIGTERM` to the file `PEER_MARKER` names. |
| `PromptExit` | Reports readiness, requires the first prompt and ends with status seven. |
| `UnicodeState` | Answers a cancelled startup dialog, then sends state with U+2028 and U+2029 inside a CRLF-terminated record. |
| `PromptAcceptance` | Answers state, emits `agent_end` and only then accepts the prompt. |
| `EndAfterAcceptance` | Accepts the prompt and streams a delta before `agent_end`. |
| `AcceptedEof` | Accepts the prompt and closes the stream before any terminal event. |
| `ExitAfterReply` | Answers the first request and exits with status three after input closes. |
| `RefusedState` | Refuses the startup request without a terminal event. |
| `EarlyEof` | Closes the stream before answering. |

`UnicodeState`, `PromptAcceptance`, `EndAfterAcceptance` and `ExitAfterReply`
require the client to close its input without another command after its last
reply, so a stray command fails the check.

#### Footer Git aliases

Run the footer observation tests with their prepared native Git aliases:

```sh
just prepare-tests
cargo nextest run --locked -p maestro-app -E 'test(/footer_/)'
```

On Unix, `maestro_test_process::git_command::Scenario` selects a compiled stand-in
behavior. Call `maestro_test_process::git_command::install(entry, scenario)` to copy
the prepared artifact and write its sibling `<alias>.fixture` record, which holds
`git` and `scenario.name()` on separate lines. Install the alias in a disposable
binary directory and put that directory first on the `PATH` of the test child that
reaches the footer operations. The two lookup scenarios answer the operands and
the physical directory their launch supplied; `GIT_EXIT` selects their status and
defaults to a successful lookup.

| Scenario | Behavior |
| --- | --- |
| `Observed` | Writes the operands, the directory and, where the platform publishes them, the descriptor targets of standard input and standard error to `GIT_LOG`; writes one diagnostic, answers `feature/native` and ends with `GIT_EXIT`. |
| `Reported` | Writes the operands and the directory to standard output, then ends with `GIT_EXIT`. |
| `Exiting` | Writes an unterminated ` prefix` and starts the descendant named by `GIT_HOLDER` on the port in `holder-port`, passing its own identifier as `GIT_PARENT`, then exits without waiting for it. |
| `Retaining` | Reads its port from `GIT_PORT`, reports the identifier in `GIT_PARENT`, waits for one release byte and adds ` tail` to the retained output. |

`Exiting` and `Retaining` keep the descriptor topology of a real linked
descendant instead of a thread: the direct child ends while its own child still
holds the output pipe, so the footer query completes only after the test releases
it. The announced identifier is the direct child's, so a test that watches for
process completion waits for the launched child rather than the descendant.
Install both aliases in one disposable binary directory:

```rust,no_run
fn main() -> std::io::Result<()> {
    let directory = maestro_test_directory::Scratch::new()?;
    let bin = directory.path().join("bin");
    std::fs::create_dir(&bin)?;
    maestro_test_process::git_command::install(
        &bin.join("git"),
        maestro_test_process::git_command::Scenario::Exiting,
    )?;
    maestro_test_process::git_command::install(
        &bin.join("git-holder"),
        maestro_test_process::git_command::Scenario::Retaining,
    )?;
    Ok(())
}
```

The installed aliases read their record instead of harness arguments, so the
product keeps its own operands, working directory and standard descriptors.

#### Search and completion aliases

On Unix, install the same artifact as `fd`, `fdfind` or `rg` to control search
output while exercising native lookup and process handling. Write `search` on
the first line of `<alias>.fixture`, followed by a JSON object with a `scenario`
field. The fixture parses the entire record before effects and rejects unknown
fields, invalid byte values and exit codes outside 0 to 255. Operands remain
native command arguments, never source text.

For example, a controlled `fd` response records the tool's operands and returns
one NUL-terminated path:

```rust,no_run
fn main() -> std::io::Result<()> {
    let artifact = std::env::var_os("MAESTRO_NATIVE_FIXTURE")
        .ok_or_else(|| std::io::Error::other("run preparation with nextest first"))?;
    let directory = maestro_test_directory::Scratch::new()?;
    let record = serde_json::json!({
        "scenario": "response",
        "args": directory.path().join("args.json"),
        "stdout": b"/work/a\0",
        "stderr": [],
        "code": 0,
        "signal": null
    });
    std::fs::write(directory.path().join("fd.fixture"), format!("search\n{record}"))?;
    maestro_test_process::install_executable(
        std::path::Path::new(&artifact), &directory.path().join("fd"),
    )?;
    Ok(())
}
```

The finite scenarios use these fields:

| Scenario | Fields and behavior |
| --- | --- |
| `response` | `args` records the JSON operand array; `stdout` and `stderr` are byte arrays. `code` selects the status; optional `signal: "SIGTERM"` ends by native signal instead. |
| `supplied` | Writes `{"argv": [...], "cwd": ...}` to `observation`. Reads the two streams and numeric status from the files named by `stdout`, `stderr` and `code`, so a test can change the outcome between searches. If the file named by `marker` exists, removes its own executable before reporting, as when a running search's installation is replaced. |
| `probe` | `name`, optional `log`, `stdout` byte array and `code`. Appends `name` and the operands to the log, requires exactly `--version`, then returns the version bytes and status. Discovery checks launchability, not version compatibility. |
| `marker` | Writes `text` to `path`. With `copy_input: true`, copies the first operand's file to stdout, like a search preprocessor. Execute it once as a positive control, remove the sentinel, then assert hostile search operands never recreate it. |
| `probe-wait` | Connects to Unix `socket`, writes `ready`, then waits for a byte or EOF. Dropping discovery must stop this real process. |
| `find-term` | Registers TERM before readiness, writes `ready` on `socket`, reads the queued TERM notification, writes `term`, then retains its output pipes until release. |
| `grep-term` | On `socket`, writes `R`, requires `x`, writes `event` bytes to stdout, optionally writes `S` when `started` is true, waits for TERM, writes `T`, requires another `x`, then exits 9. |
| `chunks` | On `socket`, writes `R` and requires `x` before each entry of `chunks`. Each entry has `stream: "stdout"` or `"stderr"` and a `bytes` array. Exits with `code` after the final write. |
| `chosen-shell` | Writes `chosen-shell` and a newline, then replaces itself with `/bin/bash`, passing the original operands unchanged. This keeps configured-shell qualification on the real shell. |
| `completion-wait` | `stdout` bytes and a parent-owned `port`. Writes the bytes, connects, writes `READY`, then waits for a byte or EOF, so a cancelled search must end this child without further output. |
| `completion-stream` | `stderr`, a count of repeating `x` bytes, and `stdout` bytes. Fails unless standard input is already closed, fills the diagnostic pipe, then writes standard output. |
| `completion-descendant` | `port` and `holder`, the alias whose record runs `completion-holder`. Reports its process id, starts the holder with inherited pipes, then exits zero while the holder keeps them. |
| `completion-holder` | `port`. Writes `D` on the parent's port and holds both inherited pipes until the test releases it with one byte. |

Attachment completion uses the four `completion-*` scenarios under any name the
test owns. The child is launched through the product operation that owns it, so
the test observes real pipes, a real status and a real cancellation. Install the
holder under a second name and put that path in the parent's `holder` field: the
descendant is a linked process, never a thread, and the search must not wait for
the pipes it keeps.

Keep these aliases in each test's disposable binary directory. Isolated discovery
and startup cases replace `PATH` explicitly; ordinary native find/grep corpus
cases still use the provisioned real tools. Run the complete `maestro-tools`
nextest selection to qualify both routes. A missing real tool fails the native
cases; the fixture never replaces or downloads it.

#### Command payloads

Install the same artifact as the program under test to observe command
execution without an interpreter. `stubborn`, `identity` and `blocked` need Unix.
Write `exec` on the first line of `<alias>.fixture`, followed by one JSON object
with a `scenario` field. The whole
record is parsed before any stream, socket or directory changes; unknown fields
and unknown scenarios fail first.

A `read_support::payload` call in `maestro-tools` installs the alias and writes
that record. The held-child cases connect to a parent-owned TCP port, so the
record carries `port`. For example, a child that reports readiness and finishes
after one release byte:

```rust,no_run
fn main() -> std::io::Result<()> {
    let artifact = std::env::var_os("MAESTRO_NATIVE_FIXTURE")
        .ok_or_else(|| std::io::Error::other("run preparation with nextest first"))?;
    let directory = maestro_test_directory::Scratch::new()?;
    let record = serde_json::json!({"scenario": "holder", "port": 9099});
    std::fs::write(directory.path().join("child.fixture"), format!("exec\n{record}"))?;
    maestro_test_process::install_executable(
        std::path::Path::new(&artifact), &directory.path().join("child"),
    )?;
    Ok(())
}
```

| Scenario | Fields and behavior |
| --- | --- |
| `operands` | Requires closed standard input, then writes `operand|cwd|HOME` for the first operand. |
| `streams` | Writes the terminal-coloured prefix and 300000 `o` bytes to standard output, then 300000 `e` bytes and CRLF to standard error. |
| `utf8` | With `port` and `split`, writes the first `split` bytes of one four-byte scalar to both outputs, announces `READY`, waits for a byte, then writes the rest. |
| `malformed` | Writes `a`, byte FF and `b` to standard output, and an incomplete scalar to standard error. |
| `cwd` | Writes the directory this process inherited. |
| `holder` | On `port`, writes the optional `stdout` and `stderr` bytes, announces `READY`, waits for one byte, then writes `bye`. |
| `stubborn` | On `port`, writes the optional `stdout` and `stderr` bytes, registers TERM, announces `READY`, and answers each TERM with `bye` on standard output and `T` on the socket until killed, released by one byte or disconnected. Unix only. |
| `descendant` | Writes `early` and `diag`, starts a linked descendant as `MTP_ROLE=descendant`, then exits. The descendant answers each `p` with `E`, writes `late` on any other byte, and is a real child of this process. |
| `yielding` | On `port`, announces `READY` under the default TERM action and answers each `p` with `E` until another byte or EOF. |
| `exchange` | On `port`, requires one `x` byte, writes `one` to standard output, requires a second `x`, then writes `two` to standard error. |
| `raw` | Connects to `port`, writes `raw`, NUL, a colour escape, CRLF and byte FF to standard output, waits for one release byte, then writes `stderr` to standard error. |
| `interleaved` | Connects to `port`, writes bytes F0 9F, waits for one release byte, writes `err` to standard error, waits for another byte, then writes bytes 98 80 and 262144 zero bytes to standard output. |
| `termination` | Connects to `port` and writes its PID followed by a newline. After one release byte, writes `alive` and a newline; after a second byte, writes `survived` to the file of that name in its working directory. Closing the channel before either byte fails without writing the marker. |
| `identity` | Writes this process's session and process group, one per line. |
| `tree-root` | On `port`, writes `READY`, starts a linked descendant as `MTP_ROLE=tree-leaf`, waits for it to end, then waits for one control byte or EOF. |
| `tree-leaf` | On `port`, writes `READY` and holds the channel until the tree is killed. |
| `blocked` | Registers TERM, announces `READY` on the optional `port`, then waits for one termination notification and finishes. An optional `code` selects its exit code after handling TERM. To test a deadline, poll execution once on a paused runtime, wait for `READY`, then advance the clock; resume real time to collect native exit and pipe completion. |
| `grace` | On Linux, writes `held`, then writes `grace` into standard output `pid` names after one input line or EOF. Finishes after a second input line or when input closes. |

The `MTP_ROLE` variable selects a linked-descendant role for the same record; no
other selection uses it. Shell parsing stays on the real shell: select the
shell itself when the command grammar, session identity or process group is the
observation, and use a payload only for what runs inside it. Use `exec <alias>`
when the payload must keep the shell's PID, process group and open descriptors.
Streaming cases release each next write only after the sink has received the
preceding bytes; no file gate or busy loop paces the child.

#### Native IO children

Install the same artifact as the program a byte-level operation launches. Every
scenario works on every platform except `grouped`, `challenge` and `identity`,
which read process groups and sessions and need Unix.
Write `child` on the first line of `<alias>.fixture` and a JSON object whose
`scenario` selects one finite behavior. The fixture parses the whole record before
any stream, socket or descendant effect, and it rejects an unknown scenario or
field. To write both streams and exit, use the `command` family's `output`
scenario.

| Scenario | Fields and behavior |
| --- | --- |
| `pending` | Writes `00 ff e2` on standard output and `error` with a NUL on standard error, releases when one input byte arrives, then writes `82 ac` followed by that byte, writes `last` on standard error and relays the rest of standard input. Every write is acknowledged by the bytes the test reads, so no chunk boundary is assumed. |
| `arguments` | Reports the working directory and the untouched operands as one JSON array. |
| `relay` | Writes `ready` when its `ready` field is true, relays standard input to standard output until it ends, then writes the `tail` bytes on standard error. |
| `announce` | Sends its process id, eight bytes big-endian, on the parent's `port`, waits for one release byte, writes the `stdout` and `stderr` byte arrays, then exits with status 0. The required boolean `stdin` selects the release source: standard input when true, the parent socket when false. |
| `grouped` | Sends its process id and group id on the parent's `port`, waits for the release byte, then exits with status 0. |
| `challenge` | Sends its process id and session id, reads a 16-byte challenge, answers with its process id and that challenge, waits for the release byte, then exits with status 23. |
| `early` | Writes the `stdout` and `stderr` byte arrays, starts `holder` with both pipes inherited, then exits with `code` while that descendant keeps them. |
| `identity` | Writes `[process id, group id, session id]` and a newline, starts `holder` when one is named, then waits for one input byte. |

A controlled child that reports a prefix before its caller releases it:

```rust,no_run
fn main() -> std::io::Result<()> {
    let artifact = std::env::var_os("MAESTRO_NATIVE_FIXTURE")
        .ok_or_else(|| std::io::Error::other("run preparation with nextest first"))?;
    let directory = maestro_test_directory::Scratch::new()?;
    let alias = directory.path().join("child");
    maestro_test_process::install_executable(
        std::path::Path::new(&artifact), &alias,
    )?;
    let record = serde_json::json!({"scenario": "announce", "port": 5091,
        "stdin": false, "stdout": [], "stderr": []});
    std::fs::write(directory.path().join("child.fixture"), format!("child\n{record}"))?;
    Ok(())
}
```

Pass the alias absolute path as the command the native operation launches, and
seed the record with the port the parent test listens on. The `early` and
`identity` scenarios name a second alias in `holder`; install that one with a
`grouped`, `announce` or `relay` record so the descendant has its own protocol.

The native I/O component captures the active run's signal before spawning an
abort-bound child. After subscribing, it also checks whether cancellation already
happened during spawn. The abort fixture waits on standard input, so releasing
the child handle closes that input and ends the child even if signal binding fails.
Watch payloads belong to one registration: opening a watch discards any unconsumed
payload from the closed registration.

#### Clipboard bytes

Install the same artifact as a clipboard writer when an application test needs real
standard input, standard output, status or a loopback transfer:

```sh
just prepare-tests
cargo nextest run --locked -p maestro-app -E 'test(/^clipboard_(copy|images)::/)'
```

The artifact copies itself into the alias, so no system installer has to answer on
the path.

Write `clipboard` on the first line of `<alias>.fixture`, followed by a JSON object
with a `scenario` field. The fixture parses the whole record before it reads a
stream or opens a socket, and rejects unknown fields, unknown scenarios and values
outside their type.

| Scenario | Fields and behavior |
| --- | --- |
| `echo` | Copies standard input to standard output, writes the single operand to standard error, then exits with `code`. |
| `send` | Connects to `port` on the loopback address, sends standard input, closes the connection and exits. A detached writer delivers bytes after its caller returned. |
| `fill` | Writes `count` copies of `byte` to standard output, so a bounded capture reaches its exact limit. |
| `paste` | Writes `types` for a first operand of `--list-types` and `image` for any other request, as a Wayland clipboard reader does. |

For example, a capture that must fail one byte past its limit records a filler and
runs the alias the way the product would:

```rust,no_run
fn main() -> std::io::Result<()> {
    let artifact = std::env::var_os("MAESTRO_NATIVE_FIXTURE")
        .ok_or_else(|| std::io::Error::other("run preparation with nextest first"))?;
    let directory = maestro_test_directory::Scratch::new()?;
    let alias = directory.path().join("wl-paste");
    maestro_test_process::install_executable(std::path::Path::new(&artifact), &alias)?;
    std::fs::write(
        directory.path().join("wl-paste.fixture"),
        "clipboard\n{\"scenario\":\"fill\",\"byte\":7,\"count\":52428801}",
    )?;
    Ok(())
}
```

The application clipboard tests keep their own assertions. The text route checks the
forwarded operand, both captured streams and status 7; the detached route checks the
bytes its listener receives; the image route checks a capture at 50 MiB and one byte
past it.

#### Viewer, editor and trash aliases

Install the same artifact as `open`, `xdg-open`, `explorer.exe`, an editor name
or `trash` when a test observes a desktop viewer launch, an edit or a session
deletion. Write the family on the first line of `<alias>.fixture`, followed by a
JSON object that the fixture parses before any effect; unknown fields and
malformed values fail at once. `maestro_test_process::install_family` installs the
alias and writes that record.

```rust,no_run
fn main() -> std::io::Result<()> {
    let directory = maestro_test_directory::Scratch::new()?;
    let editor = directory.path().join("ed");
    let record = serde_json::json!({
        "argv": directory.path().join("argv"),
        "before": directory.path().join("before"),
        "content": b"edited\n",
    });
    maestro_test_process::install_family(&editor, "editor", &record.to_string())?;
    Ok(())
}
```

The families use these fields:

| Family | Fields and behavior |
| --- | --- |
| `browser_launch` | `port` names the parent's loopback listener. The viewer writes its own launched name and its literal operands as one JSON array, waits for the parent's `!` byte, answers `done` and exits 1. A viewer that never connects leaves the parent waiting, so install the alias before the product launch. |
| `editor` | `argv` receives the launched name and every operand, each ending at a NUL. `result` names the environment variable holding the output path for the edited filename. `before` receives a copy of the file as it was, with its permission bits, `content` the new bytes, and `copy` a mode-preserving copy. `remove` deletes the file, `directory` replaces it with a directory, `read_only_parent` makes its directory read-only on Unix. `descriptors` records the three standard descriptors' device, inode, mode and device number on Linux. `code` selects the status; `signal: "SIGTERM"` ends by the native signal on Unix instead. Unsupported platform effects fail before any file effects. |
| `trash` | `log` receives each literal operand on its own line, `remove` deletes the first operand, `stderr` supplies the captured diagnostic bytes and `code` the status. Install no alias at all to observe a missing program. |

#### Credential processes

Run the model credential rows against their prepared program:

```sh
just prepare-tests
cargo nextest run --locked -p maestro-models -E 'test(/cloud_profile_credentials/)'
```

Install the same artifact as the program that an AWS profile names in
`credential_process`, or as the executable source of a Google external account
credential. Write `credentials` on the first line of `<alias>.fixture`, followed
by the scenario name. The fixture parses the whole record before it counts a run
or writes output, so an unknown record fails with no document.

| Scenario | Behavior |
| --- | --- |
| `AwsProcess` | Counts the run in `process-count` beside the alias, then answers as its single operand says. No operand and `ok` print a complete credential document. `fail` ends with status 7 and no output; `once` fails the first run only; `invalid_json` prints unusable text; `wrong_version`, `missing_key` and `expired` print a document the reader must refuse. |
| `GoogleExecutable` | Prints one success document naming a subject token and takes no operand. |

Call `maestro_test_process::credentials::install(entry, scenario)` from the
parent test, which holds `MAESTRO_NATIVE_FIXTURE`. The alias path is anything the
backend can launch; an AWS profile field names it with one variant, such as
`credential_process = /scratch/process once`. Read `process-count` back after the
run to state how often the backend started the program.

```rust,no_run
fn main() -> std::io::Result<()> {
    use maestro_test_process::credentials::{Scenario, install};
    let directory = maestro_test_directory::Scratch::new()?;
    let process = directory
        .path()
        .join(format!("process{}", std::env::consts::EXE_SUFFIX));
    install(&process, Scenario::AwsProcess)?;
    std::fs::write(directory.path().join("process-count"), "")?;
    std::fs::write(
        directory.path().join("config"),
        format!("[profile selected]\ncredential_process = {} once\n", process.display()),
    )?;
    Ok(())
}
```

The model profile rows install an `AwsProcess` alias named `process` in each
row's directory and read its count beside the file they compare. The Google
credential tests install a `GoogleExecutable` alias as the executable source of
an external account credential. Neither program needs an interpreter or an
`install` command.

#### Notification and repository Git aliases

Install the same artifact as `notify-send` or `git` when a test must observe the
operands the product hands to those programs.

| Family | Fields | Behavior |
| --- | --- | --- |
| `notify-send` | `args`, `code` | Writes each untouched operand on its own line to `args`, records nothing when `args` is null, then ends with `code`. |
| `git`, scenario `Probe` | `cwd` | Answers the three queries that repository discovery runs against a reftable checkout: `symbolic-ref --quiet --short HEAD` reports `topic`, `rev-parse --verify HEAD` reports `0123456789abcdef`, and `rev-parse --verify missing` writes output the caller discards and ends with a failing status. Every query must name `cwd` after `-C`; another directory, a non-text operand or any other operands end with status 2 before output. |

For the repository probe, write `git` and `Probe` on separate lines, followed by
the JSON object containing `cwd`. This portable scenario shares the Git family
with the Unix footer scenarios above.

A failing notifier records nothing:

```rust,no_run
fn main() -> std::io::Result<()> {
    let artifact = std::env::var_os("MAESTRO_NATIVE_FIXTURE")
        .ok_or_else(|| std::io::Error::other("run preparation with nextest first"))?;
    let directory = maestro_test_directory::Scratch::new()?;
    let record = serde_json::json!({ "args": null, "code": 1 });
    std::fs::write(
        directory.path().join("notify-send.fixture"),
        format!("notify-send\n{record}"),
    )?;
    maestro_test_process::install_executable(
        std::path::Path::new(&artifact), &directory.path().join("notify-send"),
    )?;
    Ok(())
}
```

A recording notifier names its file in `args` instead, and a Git alias names the
checkout every query must carry. Point `PATH` at the directory holding either
alias, so the product finds it by its own program name.

#### Remote client aliases

On Unix, install the same artifact as `ssh` when a test exercises an extension's
remote operations. Write `ssh` on the first line of `<alias>.fixture`, followed by
one JSON object. The fixture parses the whole record before any effect and rejects
an unknown field.

| Field | Behavior |
| --- | --- |
| `refusal` | `destination`, `code` and `stderr`. A launch naming that destination writes `stderr` and ends with `code`, so the extension's own failure path runs. |
| `write` | `command`, `code` and `stderr`. A launch whose remote command contains `command` writes `stderr` and ends with `code` without reading standard input, so the body a caller streams is never consumed. The value is a substring, because an extension quotes and resolves the path its command writes. |
| `directory` | The remote session's working directory. Every other command runs there, through `/bin/sh -c`, with the operands after the destination joined by spaces, so remote `pwd`, `cat`, `test`, `mkdir` and `file` behave as the extension expects. |

A lone leading option is skipped; an option taking a separate value is not read,
so pass the destination first. A launch left with no destination ends with status
2 and writes nothing.

```rust,no_run
fn main() -> std::io::Result<()> {
    let directory = maestro_test_directory::Scratch::new()?;
    let session = directory.path().join("session");
    std::fs::create_dir(&session)?;
    let record = serde_json::json!({
        "refusal": {"destination": "unreachable.invalid", "code": 255, "stderr": "refused\n"},
        "write": {"command": "early-write.txt", "code": 38, "stderr": "refused\n"},
        "directory": session,
    });
    maestro_test_process::install_family(
        &directory.path().join("ssh"),
        "ssh",
        &record.to_string(),
    )?;
    // Put that directory first on the child's PATH, then run the extension operation.
    Ok(())
}
```

#### Checked reruns

A test that re-runs one test in a fresh process starts it through
`maestro-test-process`, a native dev-dependency, instead of building a `Command`
for the test executable. `TestProcess::current(leaf)` resolves the running test's
full name, `TestProcess::named(identity)` selects a sibling by its complete name
(derive it from the function item with `test_name`), and the consuming
`output`, `status`, `spawn`, `status_with_stdout` and `output_with` operations
run it. The owner appends the exact filter and harness flags, confirms the test
is listed and eligible (an ignored test runs only after `.ignored()`), and checks
that the child's standard output begins with the one-test start prologue. A stale
or misspelled name therefore fails instead of passing with zero tests. The listing
behind that check is read once per executable and reused, so a test that starts
many children asks the harness once, not once per child. A checked
outcome proves the test started, not that it passed: keep exit, signal and product
assertions in the test.

Every re-run writes the prologue, including a descendant that a child starts the
same way. When several of them share one captured output, a byte count or capture
limit allows one prologue per process. A limit sized for one prologue trips on the
second, before the scenario's own output.

With `nocapture` and one test thread, the harness can put `test <name> ... `
before the child's first report on the same line. A live reader that waits for a
marker uses `split_once` to find it after that label, rather than requiring it at
the start of the line. Exercise that reader with `single_threaded()` so it also
works when the runner admits only one test thread.

Choose the route by stream use. `output` captures both streams and returns them
unchanged. `spawn` reads only the prologue from a piped standard output and replays
it, preserving ready lines and acknowledgements. A test that depends on closing
a live pipe end must first call `run_alone(test)` and return when it returns
`true`: the transaction then runs in a fresh process with no sibling tests that
could fork and inherit its pipe ends. A passing isolated run writes what its body
wrote to standard error onto the run's own standard error, such as the premise that
left a case out, through the stream handle rather than the print macros the harness
keeps: `cargo test` writes that line to the terminal, and a runner that captures the
test process, as `just test` does, shows it on a failure or when asked for a passing
test's output. `status`
replays captured streams afterwards and is not a live route. `status_with_stdout`
requires an empty regular file with its cursor at zero, sends standard output
there and checks the prefix through that same open file. Nonempty files or nonzero
cursors are rejected before launch, including append-mode files containing earlier
output. `output_with` hands the final
argument list to production code under test and accepts no environment, directory,
descriptor or setup setting. `setup` (Unix) carries an `IsolatedSetup` request.
The child applies its effects in a fixed order: umask, new session, nonterminating
file-size handling, then process-count limit. The builder writes that request into
the child's environment and the child applies it through
`IsolatedSetup::apply_requested` before its body, holding the returned guard. The
runner keeps its own modes, limits, session and environment, and a refused effect
stops the child before anything is observed. The umask contains only octal permission
bits (`000` through `777`); the child checks its conversion to the platform's native
mode type before applying it.

Child-test suites use `TestProcess::current` for the running test or
`TestProcess::named` for a supplied complete identity. `output()` checks that the
selected test starts; callers still check its exit status and scenario output.
`env_clear()` preserves the inherited coverage profile destination and the
agent directory the run's setup exports, and later caller edits can
override or remove either. Live protocols retain `CheckedChild` and
its replaying `CheckedStdout` until the exchange completes, then check successful
reaping. Call `CheckedChild::start` when the test uses its own command to launch
the selected child; it proves the same prologue and reaps a failed start. It never
signals that child: it closes the child's input and standard output and waits for
it to exit by itself. The child's standard error stays open until it ends, and the
failure then carries what the child wrote there, so it says why the selected test
did not start; a platform that reads a pipe only by waiting for it keeps no
standard error. Settings helpers select supplied complete nested identities with
`named` and request a process-count limit through `setup`, keeping reports separate
from exit status checks.

A child that a signal stops, from the test or from the code under test, waits for
that signal and never ends on its own. A coverage run writes each process's profile
as the process exits. A stop that arrives during that write leaves the profile
empty or cut. The coverage merge skips an empty profile without a warning, but a
cut one makes it refuse the whole shard. The process tests' children use
`wait_for_signal` in `crates/maestro-process/tests/child_programs.rs`. A child that
ends normally in other cases waits only when a variable asks for it: the bounded
capture child does so for the rows that overflow, so the capture stops a waiting
child, never an exiting one.

Cases that read process variables go through `run_per_environment(test, cases,
environment)`. It groups the cases by the edits `environment` returns and
reruns the test once per group, in a child that applies exactly those edits
after `env_clear()`, plus the agent directory the run's setup exports
and the parent's temporary directory, unless the case sets one. That directory
comes from `maestro-test-directory::temporary_root`. A value of
`Some` sets its variable and `None` removes it, so a case that names a kept
variable as `None` reaches its child without it. A group child
that loads a component therefore reuses the compiled code the run prepared
instead of compiling it again. The parent starts every group's child before it collects the first, so all group
children run at once and a test takes the time of its slowest group. Nextest's
thread count does not count these children.
In the parent, `run_per_environment` returns
`None` once every child passes, and repeats each passing child's standard error
in group order, so a case that left one out keeps its reason. `run_per_environment`
writes those lines through the standard error handle instead of the print macros the
harness keeps: `cargo test` writes them to the terminal, and a runner that captures
the test process, as `just test` does, shows them on a failure or when asked for a
passing test's output. In a child it returns that
group's cases in their original order, and the test checks them. Cases that share
variables therefore share one child and one fixture parse, so don't spawn a child
per case yourself. A case that needs a fresh process for other state, such as a
process-wide counter or files created per case, keeps its own child.
`with_isolated_home` runs a case in one cleared child with a scratch home and the
caller's variable edits; `with_environment` and `with_environment_without` do the
same when the case adds or clears named variables. Use
`TestProcess::case(id)` to select that case. Set a
child's temporary directory with `TestProcess::temp_dir(path)`, which writes the
variables each platform reads, never `TMPDIR` alone. A child that replaces `HOME`
and loads a prepared component passes `run_agent_directory()` as
`MAESTRO_CODING_AGENT_DIR`, so it reuses the setup's compiled code instead of
compiling the component again. A test that gives an executable its own agent
directory links the run's cache into it first with
`maestro_test_process::link_files`, as the root-command harness in
`crates/maestro/tests/support/mod.rs` and the tool-override example tests do. An entry that
a writer renames away during the link is left out, and the test that needs that component
compiles it.
Test code that proves a diagnostic path survives a closed standard error uses
`maestro_test_process::run_with_broken_stderr`, which runs the case in a child whose
standard error is a pipe with no reader, and `maestro_test_process::wait_for_broken_stderr`,
which the child calls first so no write beats the pipe's closing. The parent calls
`maestro_test_process::run_alone` before either, so no sibling test forks while the
pipe's reading end is open. The [diagnostics guide](diagnostics.md#testing-a-diagnostic-path)
shows the three together.
Read child roles through `maestro_test_process::selection()`, not environment
variables. `selection.is_child()` distinguishes the parent from a child.
`selection.runs_case(id)` accepts the parent, wildcard children and the child
selected for that id. `selection.runs_group(test)` accepts the parent, wildcard
children and environment-group children of that test. Case children do not run
groups, and group children do not run individually selected cases.
Use case identifiers for suite roles and nested stages, read the selected one with
`selection.case()`, keep fixture data in its own variable, and call `env_clear()`
before `case(id)` when clearing is needed. The owner spells that selection variable
once. A quoted `MAESTRO_*_CHILD` or `MAESTRO_*_CASE` name anywhere else under
`crates/`, in a file of any type and whether it sits in an expression or behind a
`const`, fails the owner's own guard and names every site it found.

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

### Compile cache snapshot

Shared CI restores a compiled-cache snapshot in each Linux job, but only pushes
to `main` save one. The full suite after landing runs on every push to `main`
and saves the snapshots that a merge group's preparation job restores: the
author component's compiles, and beside them the prepared test setup bundle. The
test shards only download the artifact that job builds. No caller names a
`concurrency-key`: the key names the coverage manifest, and merge groups search the
manifest of the default key as their baseline, which a main push run publishes only
under that key.

## Full-suite runs after landing

`Full suite after landing` runs the shared Rust CI on the exact commit of every
push to main. It runs all test shards, doctests and declared Chromium tests,
even when the merge queue selected fewer crates. Later pushes do not cancel
or replace queued landing runs. This is separate from required merge checks.

Push runs never read a coverage baseline: the lookup runs only for merge
groups. The suite job grants `actions: write`. After each shared job that saves
a cache, a pruning job deletes the entries that save replaced, so the cache
keeps one entry per prefix instead of filling its 10 GiB limit. Those pruning
jobs are the only ones that take write access, and they run no repository code.
GitHub checks every called job's permissions when the run starts, so a caller
that grants less makes the run fail to start. The suite caller, the reporter
caller and the guards caller pin the released shared CI commit that the
organization rulesets require. Each pin
moves with every shared-CI release re-pin
([Orchestration-Maestro/.github#231](https://github.com/Orchestration-Maestro/.github/issues/231)).

`Failed landing reports` listens for the completed main-push run. The shared
reporter creates one failure issue per landing commit with the merged PR,
failed-test log lines and run link, then proposes a revert PR against main.
The revert names the original PR and failure issue. It runs normal required
checks and enters the merge queue like any other PR; automation never pushes
the revert directly to main. Repeated handling reuses the existing issue and
open or closed revert PR, including when an earlier attempt only pushed the
revert branch. When the merge queue squashes several PRs in one push, only the
last commit has a suite run: the issue then names every PR in that push, newest
first, and the reporter fails with the run link without a revert. Revert the
culprit in a normal reviewed PR. Passing suites create neither artifact.

The reporting job needs read access to Actions runs and contents. Writes use
the existing release App's repository-scoped token, minted from
`RELEASE_APP_CLIENT_ID` and `RELEASE_APP_PRIVATE_KEY`, with contents, issues and
pull requests write access, plus workflows write access so it can revert CI
changes. The App must be installed here and the organization
secret available here. This token lets the revert trigger checks; test jobs do
not receive it. The reporting workflow is declared in
`privileged_trigger_workflows` for the existing trigger audit.

Unknown PR attribution, missing permissions or a revert conflict fails the
reporter with the original run link. The reporter also stops without an issue
or a revert when no earlier full-suite run on main bounds the push (the first
landing), or when the run did not pass but has no failed test, doctest or
Chromium job, such as a cancelled run. Inspect that run and the failure issue;
resolve a conflict in a reviewed PR. Do not bypass required checks or push to
main to recover. The workflow-run listener becomes active after it lands on
the default branch.

## CI speed guards

`CI speed guards` hands each finished `Rust CI` and `Full suite after landing`
run to the shared guards with that run's id. The guards hold time per unit of
work against a ratchet, the compile-cache hit rate, the test selection share,
shard balance, tests over 30 seconds and the pinned required checks; each one
fails a check or files one issue per cause, and a breach that persists adds to
that issue rather than filing another. A cold compile cache shows in the merge
group first: its archive build reports the hit rate against the floor.

Three kinds of run reach the guards: merge-group and pull-request runs of
`Rust CI`, and the main-push runs of `Full suite after landing`. The selection
share guard reads the pull-request runs, where a change selects crates to test.
The macOS test shards run only in the full suite after landing, so its runs
alone judge their balance.

A landing runs the suite through one reusable job, so its checks are named
`suite / Test shard (macos-latest 1/4)` and so on. The guards read the job names
behind that prefix, so a landing judges the checks it runs instead of reporting
every pinned one missing. A watched workflow this repository declares therefore
has to call the shared CI.

On the caller's own `pull_request` trigger no run id is passed, so only the
shared test-time exception check runs. It fails when a nextest override allows
more than the hang limit and the comment above it names no open issue. The
caller reads Actions runs and contents, and writes issues with the automatic job
token; GitHub reads a called workflow's own permissions when the run loads, so
one caller job covers every event. It is declared in
`privileged_trigger_workflows` beside the reporter, so the trigger audit exempts
only these two files. Like the reporter, it becomes active after it lands on the
default branch.

## Fuzzing

`fuzz/` holds coverage-guided fuzz targets for the parsers that read untrusted
text. It is a separate Cargo workspace, so `just check`, `just test` and shared
CI never build it. Each target calls a public entry point and reports a crash,
a hang or runaway memory as a finding.

| Target | Entry point |
| --- | --- |
| `markdown` | `maestro_markdown::parse_with_options` with every caller grammar, then the terminal `Markdown` component rendering an assistant reply at width 80 |
| `frontmatter` | `maestro_resources::parse_frontmatter`, which the skill and prompt loaders call |
| `terminal-escapes` | `maestro_terminal_emulator::Terminal::write` for program output and `maestro_tui::StdinBuffer::process` for keyboard input, with the same bytes |
| `protocol-frames` | `maestro_protocol::frames::read_message`, decoding consecutive client and server frames |
| `tool-call-arguments` | `maestro_models::parsed_arguments` |
| `sandbox-lowering` | `maestro_web::components::sandboxed_iframe::lowering::{program, provider}` for supplied bodies and provider expressions |
| `git-worktrees` | `maestro_daemon::git::checkouts::ExistingWorktree::parse_porcelain`, retaining newline-framed checkout path bytes |

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
| `sandbox-lowering` | 4096 | 10 | 2048 |
| `git-worktrees` | 4096 | 10 | 2048 |

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

### Typed test helpers

Shared typed helpers can use an owner's non-default `test-support` feature.
Consumers activate it through development dependencies. Owners can use a self
development dependency with `path = "."` and `features = ["test-support"]` to keep
integration tests runnable by default. Dedicated test libraries can also activate
it on normal dependencies. A product's own opt-in `test-support` feature can
forward one hop to a dependency's matching feature, but runtime/default features
cannot activate it. The conventions checker reads Cargo metadata to enforce
these boundaries while retaining cross-crate cycle detection.

The terminal toolkit's `test_support` module owns controlled runtimes, recording
terminals and input-global admission. The scenario library reexports the runtime
and terminal helpers. Theme, tool, credential, request (usage, target models and
authored schemas), agent, extension, Markdown and package helpers live in their
type owners. The extension engine's `test_support` module owns the callback gate
and the release receipts its host witnesses await. The application's `test_support`
module owns the scripted session harness, the session runtimes over it, the export
fixtures and the task-set runners. Native loopback servers, child
probes, toolchain selection and the caller-owned local executor live in
`maestro-test-process`. Fallible helpers return their errors to the test caller;
fixed-output callbacks retain immediate assertions.

Coverage probes reuse the prepared native target, workspace lock and build
profiles. Only the parent program receives coverage instrumentation.

# Browser author builds

Run `mise install` and `rustup target add wasm32-unknown-unknown`, then:

```sh
mise exec -- just browser-build
mise exec -- just browser-example-preview --port 9000
```

This builds a real editable WebAssembly **author fixture**, not a shipped browser
application or installable package. The standalone Cargo package is under
`crates/maestro-tooling/tests/fixtures/browser_author/`, outside the workspace
member list. The complete embedding application is the
[browser chat example](browser/example.md). Product packaging and browser
interaction qualification remain tracked in [#111](https://github.com/Orchestration-Maestro/maestro/issues/111)
and [#825](https://github.com/Orchestration-Maestro/maestro/issues/825).

## Commands

| Recipe | Operation |
| --- | --- |
| `browser-build` | Build both library and example outputs, stopping on failure |
| `browser-library-build`, `browser-example-build` | Locked optimized Trunk builds of the same author inputs |
| `browser-clean`, `browser-example-clean` | Remove only the respective prepared distribution |
| `browser-dev`, `browser-example-dev` | Trunk rebuilds shared inputs and serves the editable example on loopback |
| `browser-dev-compile` | Trunk watch without a server |
| `browser-check` | Apply formatting, then strict wasm Clippy for the delivered library and fixture, then example checking |
| `browser-example-check` | Locked wasm Cargo check without distribution emission |
| `browser-example-preview` | Miniserve prepared example bytes with HTML route fallback; no compilation |
| `browser-debug-build` | Separate debug HTML target, retaining source locations and declarations without wasm-opt |

Outputs live under `target/maestro-browser/{library,example,debug}`. Optimized
builds run wasm-opt; debug output does not certify a browser debugging session.
Clean preserves authored files, shared fonts, Cargo caches, preparation inputs and sibling outputs.
Native tool diagnostics and failures propagate through the recipes.

Build, check and watch recipes accept positional `pack` and `mode` inputs:

```sh
mise exec -- just browser-example-dev assets/brand/brand.json light --port 9000
```

Defaults select the checkout's brand pack and dark appearance. Supply both
positional inputs before additional native options. Native options are forwarded
by build/watch/serve/debug recipes; preview accepts only native server options.
Relative pack paths resolve from the recipe directory. Paths containing spaces,
apostrophes and non-ASCII characters remain literal arguments. One `maestro-tooling`
operation prepares and dispatches all author checks, builds and watch routes.
The launcher selects the invoking checkout from Cargo's runtime manifest directory,
falling back to its build-time checkout when that variable is absent.
A locked native `maestro-theme` example uses the existing loader/projection and writes
three presentation files plus pack/mark directory observations; the fixture has
no native path dependency or build script.

Root `dev` composes `native-dev` with `browser-dev` using just's native parallel
dependencies. Root `dev-compile` composes models with `browser-dev-compile`;
`models-dev-compile` still selects only models. Trunk owns browser watching,
rebuilding and serving; watchexec retains the native compiler graph.
Preparation creates route-specific inputs and `Trunk.json` configurations under
`target/maestro-browser/.author/{library,example,debug}/`, outside the replaceable
distributions. Checks may create these directories without creating distributions.
The generated configuration uses absolute original HTML/watch paths and retains
the authored asset paths and tool pins. Trunk
invokes the same locked native exporter through a pre-build hook on each rebuild,
retaining authored hooks alongside it. Authored hooks retain their original
working directory, command arguments, platform overrides and inherited Trunk
environment; a failed hook fails the native build.
The initial watch configuration retains authored watch roots and ignores,
resolving relative paths from the authored configuration's directory. It excludes
the generated preparation tree from rebuilds, retains the fixture and default
brand roots and adds the containing directories of both the authored paths and
symlink-resolved targets of the selected pack and mark template, including
external directories. Recursive directory watching covers edits to linked targets,
repeated replacement saves and new dependencies beneath those
roots. If you change the template to a path outside the watched directories,
restart the serve or watch command ([tracked follow-up](https://github.com/Orchestration-Maestro/maestro/issues/1088)).
The native watch option replaces configured roots rather than extending them, so the configuration is prepared from the existing theme loader;
there is no second pack decoder.

## Locks, writes and direct Cargo use

Preparation/builds write only Cargo outputs/caches, route-owned author inputs and
configuration, and the requested distributions. They do not rewrite authored
Rust, manifests, locks, identity files or borrowed Cargo configuration. The explicit
`browser-check` formatter is the sole intentional authored-source write.

The fixture keeps its own adjacent reviewed lock. Unrelated root dependency changes
need no fixture lock update; changes to fixture dependencies need explicit reviewed
lock maintenance. Native exporter/adapter builds and metadata use the root lock;
direct fixture checks use its authored manifest, `--locked` and the wasm target.
Only the Trunk child receives the locked-metadata Cargo adapter in its `CARGO`
environment: metadata gains `--locked` while native arguments, diagnostics and
status survive. Native build and artifact queries retain Trunk's locked behavior.
A stale lock fails without repair, including through actual Trunk.

For direct fixture Cargo work, prepare the selected inputs first, then provide
`MAESTRO_BROWSER_INPUTS` pointing to their absolute directory:

```sh
cargo run --locked -p maestro-theme --example browser_author_inputs -- \
  "$PWD/assets/brand/brand.json" dark "$PWD/target/maestro-browser/.author/example/inputs"
MAESTRO_BROWSER_INPUTS="$PWD/target/maestro-browser/.author/example/inputs" \
  cargo check --locked --manifest-path crates/maestro-tooling/tests/fixtures/browser_author/Cargo.toml \
  --target wasm32-unknown-unknown
```

Before tests, explicitly provision both independent locks:

```sh
cargo fetch --locked
cargo fetch --locked --manifest-path crates/maestro-tooling/tests/fixtures/browser_author/Cargo.toml
```

Shared CI declares the fixture through `additional_locked_manifests` without
activating the optional product browser-build job. Cold-cache tests stage complete
local sources with root `cargo vendor --locked --offline --sync <fixture-manifest>
<owned-source-directory>`. Their child starts with empty Cargo home/target and a
test-owned directory-source/offline configuration, not a copied registry/index or
build cache. Missing local sources fail visibly; setup never repairs locks or
fetches during the cold phase.

## Tools and prepared metadata

Mise pins Trunk **0.21.14**, wasm-bindgen-cli **0.2.129**, Binaryen/wasm-opt
**133** and miniserve **0.35.0**, with release digests in `mise.lock`. Rust stays
pinned by `rust-toolchain.toml`; just is **1.58.0**. These are external author
tools, not product Cargo dependencies or consumer installation hooks. See the
[tool library inventory](libraries/repository-tooling.md#external-browser-author-tools).

`package.json.in` is a metadata template only. Its module, declaration,
stylesheet and version placeholders must be resolved by real product packaging;
copying it into output does not make a publishable package. It has no lifecycle
scripts. Shared CI's optional product browser-build activation remains off.

## Fonts and notices

`assets/brand/fonts/` holds six unmodified whole files: Barlow Regular, Barlow
Condensed Semibold/Bold, JetBrains Mono Regular v2.304, Space Grotesk variable
and Noto Sans Symbols 2 Regular. Barlow and Space Grotesk originate from their
Google Fonts family distributions; JetBrains Mono comes from the official
[v2.304 release](https://github.com/JetBrains/JetBrainsMono/releases/tag/v2.304).
Noto Sans Symbols 2 comes from Google Fonts commit
`2750b521bd22ce141f12617194ab0e04cf5d12d5`,
[`ofl/notosanssymbols2/NotoSansSymbols2-Regular.ttf`](https://github.com/google/fonts/blob/2750b521bd22ce141f12617194ab0e04cf5d12d5/ofl/notosanssymbols2/NotoSansSymbols2-Regular.ttf).
The five original family notices travel with prepared assets; see the
[notice inventory](../THIRD-PARTY-NOTICES.md#browser-author-fonts-and-tools).

Font roles come from the selected pack. The fixture stylesheet registers each
file under its family name, and the page's font stacks keep the pack's fallback
order. Noto is a symbol fallback, not a fifth brand role. The fixture's wasm
graph needs no subsetter or remote font service.

The six fonts total **1,967,112 raw bytes**; qualification measured **881,586
bytes** with deterministic gzip.

The fixture mounts the mark and prepared page styling beside a native text
input with browser-default paint. Native tests read files and bytes; they do
not render glyphs or certify browser pixels.

## Validation boundary

Focused `browser_author_build` tests exercise actual optimized/debug
builds, strict checking, independently edited Rust/CSS/shared identity inputs,
native serving, external pack/mark rebuilds on serving and compile-only routes,
formatting repair, prepared-only preview and child failures. Controlled command
fixtures cover command selection and finite negative traces, not rendering.
Native tests also inspect assets, wasm magic, bindings and source locations.
Browser interaction and startup performance are not inferred from native server
readiness or binary inspection.

Measured raw / deterministic gzip bytes from the qualified focused run:

| Output | Raw bytes | Gzip bytes |
| --- | ---: | ---: |
| Optimized distribution | 2,023,703 | 908,491 |
| Optimized wasm | 22,855 | 9,829 |
| Debug distribution | 4,026,669 | 1,265,151 |
| Debug wasm | 2,025,821 | 366,490 |
| Shared font directory, including five notices | 1,989,050 | 894,890 |

These are observations, not size budgets or startup-speed claims. Build paths
and tool output may change compressed sizes. Shared CI supplies workspace tests
and line coverage; no local coverage or mutation run is required.

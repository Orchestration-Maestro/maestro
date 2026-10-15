# Browser author builds

Run `mise install` and `rustup target add wasm32-unknown-unknown`, then:

```sh
mise exec -- just browser-build
mise exec -- just browser-example-preview --port 9000
```

This builds a real editable WebAssembly **author fixture**, not a shipped browser
application or installable package. The standalone Cargo package is under
`crates/maestro-tooling/tests/fixtures/browser_author/`, outside the workspace
member list. Product packaging, the complete embedding application and browser
interaction qualification remain tracked in [#111](https://github.com/Orchestration-Maestro/maestro/issues/111),
[#823](https://github.com/Orchestration-Maestro/maestro/issues/823) and
[#825](https://github.com/Orchestration-Maestro/maestro/issues/825).

## Commands

| Recipe | Operation |
| --- | --- |
| `browser-build` | Build both library and example outputs, stopping on failure |
| `browser-library-build`, `browser-example-build` | Locked optimized Trunk builds of the same author inputs |
| `browser-clean`, `browser-example-clean` | Remove only the respective prepared distribution |
| `browser-dev`, `browser-example-dev` | Trunk rebuilds shared inputs and serves the editable example on loopback |
| `browser-dev-compile` | Trunk watch without a server |
| `browser-check` | Check formatting and strict wasm Clippy for the delivered library and fixture, then example checking |
| `browser-example-check` | Locked wasm Cargo check without distribution emission |
| `browser-example-preview` | Miniserve prepared example bytes with HTML route fallback; no compilation |
| `browser-debug-build` | Separate debug HTML target, retaining source locations and declarations without wasm-opt |

Outputs live under `target/maestro-browser/{library,example,debug}`. Optimized
builds run wasm-opt; debug output does not certify a browser debugging session.
Clean preserves authored files, shared fonts, Cargo caches and sibling outputs.
Native tool diagnostics and failures propagate through the recipes.

Build, check and watch recipes accept positional `pack` and `mode` inputs:

```sh
mise exec -- just browser-example-dev assets/brand/brand.json light --port 9000
```

Defaults select the checkout's brand pack and dark appearance. Supply both
positional inputs before additional native options. Native options are forwarded
by build/watch/serve/debug recipes; preview accepts only native server options.
Paths containing spaces, apostrophes and non-ASCII characters remain literal
arguments. Preparation reads `MAESTRO_BRAND_PACK` / `MAESTRO_BRAND_MODE` through
the existing theme loader and projection; no alternate identity decoder exists.

Root `dev` composes `native-dev` with `browser-dev` using just's native parallel
dependencies. Root `dev-compile` composes models with `browser-dev-compile`;
`models-dev-compile` still selects only models. Trunk owns browser watching,
rebuilding and serving; watchexec retains the native compiler graph.

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

Font roles come from the selected pack. Physical registration names identify
files, not semantic roles. The selected body family precedes mono and symbol
fallbacks in the toolkit; DOM stacks retain the pack's fallback order. Noto is a
symbol fallback, not a fifth brand role. No subsetter, remote font service or
default toolkit font bundle is required by the fixture's wasm graph.

The six fonts total **1,967,112 raw bytes**; qualification measured **881,586
bytes** with deterministic gzip. Real glyph tests cover `✦✓✗┃↑↓` through five
family/weight chains with an empty-font toolkit context and nonempty tessellation.
This is not arbitrary-script coverage or real-browser pixel certification.

## Validation boundary

Twenty focused `browser_author_build` tests exercise actual optimized/debug
builds, strict checking, independently edited Rust/CSS/shared identity inputs,
native serving, prepared-only preview and child failures. Controlled command
fixtures cover command selection and finite negative traces, not rendering.
Native tests also inspect assets, glyph geometry, wasm magic, bindings and source
locations. Browser interaction and startup performance are not inferred from
native server readiness or binary inspection.

Measured raw / deterministic gzip bytes from the qualified focused run:

| Output | Raw bytes | Gzip bytes |
| --- | ---: | ---: |
| Optimized distribution | 7,015,341 | 2,908,185 |
| Optimized wasm | 4,947,015 | 1,999,627 |
| Debug distribution | 42,029,686 | 9,564,309 |
| Debug wasm | 39,959,850 | 8,655,476 |
| Shared font directory, including five notices | 1,989,143 | 894,937 |

These are observations, not size budgets or startup-speed claims. Build paths
and tool output may change compressed sizes. Shared CI supplies workspace tests
and line coverage; no local coverage or mutation run is required.

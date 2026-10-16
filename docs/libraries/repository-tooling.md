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

`cargo-hakari =0.9.39` (MIT OR Apache-2.0), pinned in `mise.toml`, is an external
development tool, not a Cargo dependency. It generates the marked dependency section
of `maestro-dependencies`, whose manifest therefore repeats existing locked packages
and features for native targets; it adds no package version of its own.

Build-recipe tests in `maestro-tooling` use the existing `toml =1.1.6`
(default features, MIT OR Apache-2.0) as a development dependency to copy local
`path` sources from the root manifest's `[patch]` tables into disposable build
fixtures. Registry/git patches need no local copy; production watching is
unchanged.
## External browser author tools

Mise pins Trunk 0.21.14 and wasm-bindgen-cli 0.2.129 (MIT OR Apache-2.0),
Binaryen/wasm-opt 133 (Apache-2.0), and miniserve 0.35.0 (MIT). Native release
checksums are retained in `mise.lock`. These are external development binaries,
not workspace Cargo dependencies or consumer lifecycle hooks. The checked
non-development Linux-musl closures contain 394, 168 and 296 packages for Trunk,
wasm-bindgen-cli and miniserve respectively. Trunk's mandatory
lockfree-object-pool 0.1.6 uses the Boost Software License 1.0; its
[notice](../licenses/author-tools/lockfree-object-pool.txt) is retained alongside
these author-tool records. See [browser author commands](../browser-build.md).

The bundler configuration is authored as `Trunk.json`, parsed with tooling's
existing serde_json dependency. Tooling builds the existing theme owner's native
export example and its own wiring-only locked-metadata adapter as separate locked
root operations; no theme/brand Cargo edge is added to tooling or the fixture.
Generated route configurations retain the pinned tool versions and original
absolute HTML/watch paths. Only the Trunk child's `CARGO` selects the adapter;
metadata gains `--locked`, while other invocations pass through unchanged. See the
[browser lock/write contract](../browser-build.md#locks-writes-and-direct-cargo-use)
for preparation, independent lock maintenance and offline source provisioning.

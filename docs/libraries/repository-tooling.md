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

Documentation checks use the shared `maestro-markdown` parser. It uses
`regress =0.12.0` with defaults and `prohibit-unsafe` (MIT OR Apache-2.0),
with the existing `memchr =2.8.3` dependency, and `entities =1.0.1` (MIT).
The entity table has no features or production dependencies. Reference keys use
`unicase =2.10.0` (MIT OR Apache-2.0) for full Unicode folding. Parser records
retain task markers as data; terminal formatting does not render checkboxes.

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

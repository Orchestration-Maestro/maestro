## Approved syntax highlighting libraries

`maestro-theme` uses `syntect =5.3.0` (MIT; defaults off, `parsing`, `regex-fancy`),
`two-face =0.5.2` (resolved as `0.5.2+bat-0.26.1`; MIT OR Apache-2.0; defaults off,
`syntect-fancy`; `two_face::syntax::extra_newlines()`) and `owo-colors =4.2.1` (MIT) for
terminal and escaped-HTML highlighting and decoration openings, plus the internal
`maestro-tui` edge for component style records. The pure-Rust fancy-regex engine is the
only token-painting regular-expression backend: never enable onig or onig_sys. The added closure is adler2,
bincode, fancy-regex 0.16.2, flate2, miniz_oxide, simd-adler32 and zlib-rs; the
`unicode-ident` 1.0.24 and `syn` 3.0.6 pins are unchanged. No third-party type appears in the
public interface.


Automatic HTML language selection uses `regress =0.12.0` (MIT OR Apache-2.0, using MIT),
with its defaults (`std`, `backend-pikevm`) plus `prohibit-unsafe`; its normal dependency
closure is the already-locked `memchr =2.8.3` (MIT OR Unlicense, using MIT). This is a
recognizer only, not a second token painter. Immutable grammar tables derive from
highlight.js 11.9.0 for exported sessions and 11.11.1 for browser code blocks, under
BSD-3-Clause. Each private language module records its upstream file, version and SHA-256;
identical expressions, keyword tables and modes share one owner. See the full
[derived-data notice](../../THIRD-PARTY-NOTICES.md#html-language-grammar-data).

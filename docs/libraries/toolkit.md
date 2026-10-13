## Approved toolkit libraries

`maestro-tui` alone may use `unicode-segmentation =1.13.3` and
`unicode-width =0.2.2` (both with defaults off) for grapheme boundaries and
scalar cell widths, and `regress =0.12.0` (defaults `backend-pikevm` and `std`,
plus `prohibit-unsafe`) for the Unicode property expressions of its width policy.
All three are MIT OR Apache-2.0. For the text helpers, the only added transitive
dependency is
`memchr =2.8.3` (Unlicense OR MIT; `alloc` and `std`). No type of these libraries
appears in the toolkit text-helper interfaces.

For inline images, `maestro-tui` may use `base64 =0.23.1` (defaults off,
`std`) and `rand =0.10.3` (defaults). Its browser target declares
`getrandom =0.4.3` with `wasm_js` independently. These libraries are MIT OR
Apache-2.0; base64 adds no transitive dependencies, and the existing random/browser
closure is reused. Fixture consumption uses `serde_json` only as a dev-dependency.
No third-party type appears in the image interface.

`maestro-chat` reuses `indexmap =2.14.2` with default `std` for its shared,
replaceable action map. Its direct closure is `equivalent =1.0.2` and
`hashbrown =0.17.1` without hashbrown defaults; all three are MIT OR Apache-2.0.
`maestro-chat` also uses `regress =0.12.0` with the same default features and
`prohibit-unsafe` for case-insensitive session search expressions. Its parsed
query interface exposes the compiled `regress::Regex`. It reuses the existing
`memchr =2.8.3` dependency; versions and licenses are unchanged.

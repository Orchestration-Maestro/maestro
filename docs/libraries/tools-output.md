# Tool output libraries

`maestro-tools` uses workspace-pinned `serde =1.0.229` with derive and default
`std` for output serialization. Records use camelCase field names; reasons use
lowercase strings. No output record is deserialized by the product.

Workspace-pinned `ryu-js =1.0.3` supplies number spelling and one-decimal fixed
formatting for [size text](../tools-read.md#line-clipping-and-size-text). Use its
Apache-2.0 license option. It also formats image coordinate notes to two places. Serde and its derive closure have permitted MIT or
Apache-2.0 options; unicode-ident retains Unicode-3.0.

Tests use workspace `serde_json =1.0.151` with `preserve_order` and
`float_roundtrip` to compare independently recorded output trees.

Native [streaming output](../tools-read.md#native-streaming) additionally uses
`maestro-path` for lexical filename joining and `getrandom =0.4.3`, with no extra
features, for artifact identifiers. Its native dependency closure uses existing
`cfg-if =1.0.5`, `libc =0.2.190` and target-specific `r-efi =6.0.0`; use their
MIT or Apache-2.0 license options. Files and buffered writes use the standard
library. The streaming module is excluded from browser builds.

## Image libraries

Image operations use `image =0.24.9` (MIT or Apache-2.0) directly, with
`default-features = false`. The `png` and `jpeg` features supply decoding and
candidate encoding; `gif`, `webp` and `bmp` retain byte-decoder conversion inputs.
The [byte decoder](https://docs.rs/image/0.24.9/image/fn.load_from_memory.html)
provides fallible decoding. RGBA images use Lanczos3 resizing, axis flips and
integer quarter-turns from `image::imageops`; `DynamicImage::write_to` encodes
PNG or JPEG at the requested quality. The same features build on native and
browser targets, without a browser-specific codec wrapper.

`kamadak-exif =0.6.1` (BSD-2-Clause) owns TIFF/JPEG metadata decoding. Ordered
`fields()` selection uses the first retained primary orientation, with
partial-result recovery for metadata errors. The utility owns only the WebP chunk
envelope and optional prefix.
`base64 =0.23.1` uses defaults off plus `std` here (MIT or Apache-2.0); shared
workspace features also enable its defaults. `jpeg-decoder =0.3.2` (MIT or
Apache-2.0), defaults off plus `platform_independent`, enables deterministic color
conversion in the existing image decoder; the direct declaration adds no second
codec. The corresponding cargo-machete exception is feature-only.
Workspace `num-traits =0.2.19` supplies checked rounded geometry conversion;
its existing image-library dependency is reused, with no new package or version.

### Resolved image dependency closure

The table records the resolved native/browser feature union for the image,
base64, EXIF and number roots. Choose MIT or Apache-2.0 when offered (including
0BSD/Zlib/Unlicense alternatives); retain BSD, Apache-2.0 and Unicode-3.0 when
those are the applicable terms. This is dependency attribution, not an API or
extra capability.

| Package | Version | Resolved features | License expression |
|---|---|---|---|
| adler2 | 2.0.1 | none | 0BSD OR MIT OR Apache-2.0 |
| autocfg | 1.5.1 | none | Apache-2.0 OR MIT |
| base64 | 0.23.1 | alloc, default, simd-unsafe, std | MIT OR Apache-2.0 |
| bitflags | 1.3.2 | default | MIT/Apache-2.0 |
| bytemuck | 1.25.2 | extern_crate_alloc | Zlib OR Apache-2.0 OR MIT |
| byteorder | 1.5.0 | default, std | Unlicense OR MIT |
| cfg-if | 1.0.5 | none | MIT OR Apache-2.0 |
| color_quant | 1.1.0 | none | MIT |
| crc32fast | 1.5.2 | default, std | MIT OR Apache-2.0 |
| fdeflate | 0.3.7 | none | MIT OR Apache-2.0 |
| flate2 | 1.1.10 | any_impl, default, miniz_oxide, runtime_detection, rust_backend | MIT OR Apache-2.0 |
| gif | 0.13.3 | color_quant, default, raii_no_panic, std | MIT OR Apache-2.0 |
| image | 0.24.9 | bmp, gif, jpeg, png, webp | MIT OR Apache-2.0 |
| jpeg-decoder | 0.3.2 | platform_independent | MIT OR Apache-2.0 |
| kamadak-exif | 0.6.1 | none | BSD-2-Clause |
| miniz_oxide | 0.8.9 | default, simd, simd-adler32, with-alloc | MIT OR Zlib OR Apache-2.0 |
| miniz_oxide | 0.9.1 | default, simd, simd-adler32, with-alloc | MIT OR Zlib OR Apache-2.0 |
| mutate_once | 0.1.2 | none | BSD-2-Clause |
| num-traits | 0.2.19 | default, i128, std | MIT OR Apache-2.0 |
| png | 0.17.16 | none | MIT OR Apache-2.0 |
| ryu-js | 1.0.3 | none | Apache-2.0 OR BSL-1.0 |
| simd-adler32 | 0.3.10 | const-generics, default, std | MIT |
| weezl | 0.1.12 | alloc, default, std | MIT OR Apache-2.0 |
| zlib-rs | 0.6.8 | rust-allocator, std | Zlib |

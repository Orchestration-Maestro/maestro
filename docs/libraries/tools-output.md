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

## Tool definitions and read

Generic definitions and wrappers build on native and browser targets with the
internal `maestro-agent`, `maestro-models`, `maestro-theme` and `maestro-tui`
crates, workspace `serde_json` (now also a normal dependency) and workspace
`futures-util =0.3.34` (`std`, `sink`). The read family is native-only:
`icu_normalizer =2.3.0` with `compiled_data` supplies NFD, `tokio =1.53.2` with
`rt` and `fs` supplies file reads and blocking probes, and on Unix `rustix =1.1.5`
with `std` and `fs` checks readability without opening the file. Other targets
open the file. No package or version beyond the existing lockfile is added.
Read selection spells floating-point line numbers with `ryu-js`.

The home directory comes from `std::env::home_dir`: an unset or empty `HOME`
falls back to the account's home entry, where the JavaScript runtime would return
an empty string only for an empty `HOME` (an unset `HOME` also falls back there), and a missing entry is a lookup error returned only for `~` paths.

## Image libraries

Image operations use `image =0.24.9` (MIT or Apache-2.0) directly, with
`default-features = false`. The `png` and `jpeg` features supply decoding and
candidate encoding; `gif`, `webp`, `bmp`, `ico`, `pnm`, `tiff`, `hdr`, `dds`
and `farbfeld` retain byte-decoder inputs. `dds` enables `dxt` for its compressed
pixel blocks. These inputs are available to conversion and resizing independently
of the MIME detector's supported formats.
The [byte decoder](https://docs.rs/image/0.24.9/image/fn.load_from_memory.html)
provides fallible decoding. RGBA images use Lanczos3 resizing, axis flips and
integer quarter-turns from `image::imageops`; `DynamicImage::write_to` encodes
PNG or JPEG at the requested quality. The same features build on native and
browser targets, without a browser-specific codec wrapper.

`kamadak-exif =0.6.1` (BSD-2-Clause), declared as `exif`, owns TIFF/JPEG metadata decoding. Ordered
`fields()` selection uses the first retained primary orientation, with
partial-result recovery for metadata errors. The utility owns only the WebP chunk
envelope and optional prefix.
`base64 =0.23.1` uses defaults off plus `std` here (MIT or Apache-2.0); shared
workspace features also enable its defaults. The image library uses
`jpeg-decoder =0.3.2` (MIT or Apache-2.0) with defaults off and no extra features;
there is no direct decoder declaration or cargo-machete exception.

Native JPEG decoding uses runtime-selected SIMD IDCT and colour conversion,
which can differ from the scalar decoding path. The measured maximum absolute
per-channel difference across the archived eighteen-image corpus is 3. The three
reencoded JPEG-input observations compare format, dimensions and pixels within
that bound; deterministic paths retain exact encoded-byte comparisons.
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
| image | 0.24.9 | bmp, dds, dxt, farbfeld, gif, hdr, ico, jpeg, png, pnm, tiff, webp | MIT OR Apache-2.0 |
| jpeg-decoder | 0.3.2 | none | MIT OR Apache-2.0 |
| kamadak-exif | 0.6.1 | none | BSD-2-Clause |
| miniz_oxide | 0.8.9 | default, simd, simd-adler32, with-alloc | MIT OR Zlib OR Apache-2.0 |
| miniz_oxide | 0.9.1 | default, simd, simd-adler32, with-alloc | MIT OR Zlib OR Apache-2.0 |
| mutate_once | 0.1.2 | none | BSD-2-Clause |
| num-traits | 0.2.19 | default, i128, std | MIT OR Apache-2.0 |
| png | 0.17.16 | none | MIT OR Apache-2.0 |
| ryu-js | 1.0.3 | none | Apache-2.0 OR BSL-1.0 |
| simd-adler32 | 0.3.10 | const-generics, default, std | MIT |
| tiff | 0.9.1 | none | MIT |
| weezl | 0.1.12 | alloc, default, std | MIT OR Apache-2.0 |
| zlib-rs | 0.6.8 | rust-allocator, std | Zlib |

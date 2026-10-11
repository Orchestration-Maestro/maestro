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

Image operations use `photon-rs =0.3.3` (Apache-2.0), defaults off, with target-only
`enable_wasm` for browser builds. Its [native byte decoder](https://docs.rs/photon-rs/0.3.3/photon_rs/native/fn.open_image_from_bytes.html)
provides fallible decoding; its image methods provide PNG/JPEG encoding and its
transform module supplies Lanczos3 resizing and axis flips. Quarter-turn pixel
permutation belongs to the orientation utility rather than general-angle rotation.

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
| ab_glyph_rasterizer | 0.1.10 | default, std | Apache-2.0 |
| adler2 | 2.0.1 | none | 0BSD OR MIT OR Apache-2.0 |
| allocator-api2 | 0.2.21 | alloc | MIT OR Apache-2.0 |
| approx | 0.5.1 | default, std | Apache-2.0 |
| autocfg | 1.5.1 | none | Apache-2.0 OR MIT |
| base64 | 0.13.1 | default, std | MIT/Apache-2.0 |
| base64 | 0.23.1 | alloc, default, simd-unsafe, std | MIT OR Apache-2.0 |
| bitflags | 1.3.2 | default | MIT/Apache-2.0 |
| bumpalo | 3.20.3 | allocator-api2, default | MIT OR Apache-2.0 |
| bytemuck | 1.25.2 | extern_crate_alloc | Zlib OR Apache-2.0 OR MIT |
| byteorder | 1.5.0 | default, std | Unlicense OR MIT |
| cfg-if | 0.1.10 | none | MIT/Apache-2.0 |
| cfg-if | 1.0.5 | none | MIT OR Apache-2.0 |
| color_quant | 1.1.0 | none | MIT |
| console_error_panic_hook | 0.1.7 | none | Apache-2.0/MIT |
| conv | 0.3.3 | none | MIT |
| crc32fast | 1.5.2 | default, std | MIT OR Apache-2.0 |
| custom_derive | 0.1.7 | none | MIT/Apache-2.0 |
| deranged | 0.5.8 | default | MIT OR Apache-2.0 |
| either | 1.19.0 | std, use_std | MIT OR Apache-2.0 |
| fdeflate | 0.3.7 | none | MIT OR Apache-2.0 |
| find-crate | 0.6.3 | none | Apache-2.0 OR MIT |
| flate2 | 1.1.10 | any_impl, default, miniz_oxide, runtime_detection, rust_backend | MIT OR Apache-2.0 |
| futures-channel | 0.3.34 | alloc, default, futures-sink, sink, std | MIT OR Apache-2.0 |
| futures-core | 0.3.34 | alloc, default, std | MIT OR Apache-2.0 |
| futures-io | 0.3.34 | std | MIT OR Apache-2.0 |
| futures-macro | 0.3.34 | none | MIT OR Apache-2.0 |
| futures-sink | 0.3.34 | alloc, default, std | MIT OR Apache-2.0 |
| futures-task | 0.3.34 | alloc, std | MIT OR Apache-2.0 |
| futures-util | 0.3.34 | alloc, async-await, async-await-macro, channel, default, futures-channel, futures-io, futures-macro, futures-sink, io, memchr, sink, slab, std | MIT OR Apache-2.0 |
| getrandom | 0.1.16 | std | MIT OR Apache-2.0 |
| gif | 0.13.3 | color_quant, default, raii_no_panic, std | MIT OR Apache-2.0 |
| image | 0.24.9 | bmp, dds, dxt, farbfeld, gif, hdr, ico, jpeg, png, pnm, tga, tiff, webp | MIT OR Apache-2.0 |
| imageproc | 0.23.1 | none | MIT |
| instant | 0.1.13 | none | BSD-3-Clause |
| itertools | 0.10.5 | default, use_alloc, use_std | MIT/Apache-2.0 |
| jpeg-decoder | 0.3.2 | platform_independent | MIT OR Apache-2.0 |
| js-sys | 0.3.106 | default, std, unsafe-eval | MIT OR Apache-2.0 |
| kamadak-exif | 0.6.1 | none | BSD-2-Clause |
| libc | 0.2.190 | default, std | MIT OR Apache-2.0 |
| matrixmultiply | 0.3.11 | avx512, default, std | MIT/Apache-2.0 |
| memchr | 2.8.3 | alloc, default, std | Unlicense OR MIT |
| miniz_oxide | 0.8.9 | default, simd, simd-adler32, with-alloc | MIT OR Zlib OR Apache-2.0 |
| miniz_oxide | 0.9.1 | default, simd, simd-adler32, with-alloc | MIT OR Zlib OR Apache-2.0 |
| mutate_once | 0.1.2 | none | BSD-2-Clause |
| nalgebra | 0.30.1 | matrixmultiply, std | BSD-3-Clause |
| node-sys | 0.4.2 | none | Apache-2.0 OR MIT |
| num | 0.4.3 | default, num-bigint, std | MIT OR Apache-2.0 |
| num-bigint | 0.4.8 | default, std | MIT OR Apache-2.0 |
| num-complex | 0.4.6 | std | MIT OR Apache-2.0 |
| num-conv | 0.2.2 | none | MIT OR Apache-2.0 |
| num-integer | 0.1.47 | i128, std | MIT OR Apache-2.0 |
| num-iter | 0.1.46 | i128, std | MIT OR Apache-2.0 |
| num-rational | 0.4.2 | num-bigint, num-bigint-std, std | MIT OR Apache-2.0 |
| num-traits | 0.2.19 | default, i128, std | MIT OR Apache-2.0 |
| once_cell | 1.21.4 | alloc, default, race, std | MIT OR Apache-2.0 |
| owned_ttf_parser | 0.15.2 | apple-layout, default, glyph-names, opentype-layout, std, variable-fonts | Apache-2.0 |
| palette | 0.6.1 | default, named, named_from_str, named_gradients, phf, std | MIT OR Apache-2.0 |
| palette_derive | 0.6.1 | none | MIT OR Apache-2.0 |
| paste | 1.0.15 | none | MIT OR Apache-2.0 |
| perlin2d | 0.2.6 | none | Apache-2.0 |
| phf | 0.11.3 | macros, phf_macros | MIT |
| phf_generator | 0.11.3 | none | MIT |
| phf_macros | 0.11.3 | none | MIT |
| phf_shared | 0.11.3 | none | MIT |
| photon-rs | 0.3.3 | console_error_panic_hook, enable_wasm, js-sys, node-sys, wasm-bindgen, web-sys | Apache-2.0 |
| pin-project-lite | 0.2.17 | none | Apache-2.0 OR MIT |
| png | 0.17.16 | none | MIT OR Apache-2.0 |
| powerfmt | 0.2.1 | none | MIT OR Apache-2.0 |
| ppv-lite86 | 0.2.21 | simd, std | MIT OR Apache-2.0 |
| proc-macro2 | 1.0.107 | default, proc-macro, span-locations | MIT OR Apache-2.0 |
| quote | 1.0.47 | default, proc-macro | MIT OR Apache-2.0 |
| rand | 0.7.3 | alloc, default, getrandom, getrandom_package, libc, std | MIT OR Apache-2.0 |
| rand | 0.8.8 | small_rng | MIT OR Apache-2.0 |
| rand_chacha | 0.2.2 | std | MIT OR Apache-2.0 |
| rand_core | 0.5.1 | alloc, getrandom, std | MIT OR Apache-2.0 |
| rand_core | 0.6.4 | none | MIT OR Apache-2.0 |
| rand_distr | 0.2.2 | none | MIT OR Apache-2.0 |
| rand_hc | 0.2.0 | none | MIT/Apache-2.0 |
| rawpointer | 0.2.1 | none | MIT/Apache-2.0 |
| rusttype | 0.9.3 | default, has-atomics, std | MIT / Apache-2.0 |
| rustversion | 1.0.23 | none | MIT OR Apache-2.0 |
| ryu-js | 1.0.3 | none | Apache-2.0 OR BSL-1.0 |
| safe_arch | 0.7.4 | bytemuck, default | Zlib OR Apache-2.0 OR MIT |
| serde | 1.0.229 | alloc, default, derive, rc, serde_derive, std | MIT OR Apache-2.0 |
| serde_core | 1.0.229 | alloc, default, rc, result, std | MIT OR Apache-2.0 |
| serde_derive | 1.0.229 | default | MIT OR Apache-2.0 |
| simba | 0.7.3 | std, wide | Apache-2.0 |
| simd-adler32 | 0.3.10 | const-generics, default, std | MIT |
| siphasher | 1.0.4 | default, std | MIT OR Apache-2.0 |
| slab | 0.4.12 | default, std | MIT |
| syn | 1.0.109 | clone-impls, default, derive, extra-traits, parsing, printing, proc-macro, quote | MIT OR Apache-2.0 |
| syn | 2.0.119 | clone-impls, default, derive, extra-traits, full, parsing, printing, proc-macro, visit-mut | MIT OR Apache-2.0 |
| syn | 3.0.6 | clone-impls, default, derive, extra-traits, fold, full, parsing, printing, proc-macro, visit, visit-mut | MIT OR Apache-2.0 |
| thiserror | 1.0.69 | none | MIT OR Apache-2.0 |
| thiserror-impl | 1.0.69 | none | MIT OR Apache-2.0 |
| tiff | 0.9.1 | none | MIT |
| time | 0.3.55 | alloc, default, std | MIT OR Apache-2.0 |
| time-core | 0.1.9 | none | MIT OR Apache-2.0 |
| toml | 0.5.11 | default | MIT/Apache-2.0 |
| ttf-parser | 0.15.2 | apple-layout, glyph-names, opentype-layout, std, variable-fonts | MIT OR Apache-2.0 |
| typenum | 1.20.1 | const-generics | MIT OR Apache-2.0 |
| unicode-ident | 1.0.24 | none | (MIT OR Apache-2.0) AND Unicode-3.0 |
| wasi | 0.9.0+wasi-snapshot-preview1 | default, std | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| wasm-bindgen | 0.2.129 | default, std, strict-macro | MIT OR Apache-2.0 |
| wasm-bindgen-macro | 0.2.129 | strict-macro | MIT OR Apache-2.0 |
| wasm-bindgen-macro-support | 0.2.129 | strict-macro | MIT OR Apache-2.0 |
| wasm-bindgen-shared | 0.2.129 | none | MIT OR Apache-2.0 |
| web-sys | 0.3.106 | AbortController, AbortSignal, Blob, BlobPropertyBag, CanvasRenderingContext2d, CssStyleDeclaration, Document, Element, EventTarget, File, FormData, Headers, HtmlCanvasElement, HtmlElement, HtmlImageElement, ImageData, Node, QueuingStrategy, ReadableByteStreamController, ReadableStream, ReadableStreamByobReader, ReadableStreamByobRequest, ReadableStreamDefaultController, ReadableStreamDefaultReader, ReadableStreamGetReaderOptions, ReadableStreamReadResult, ReadableStreamReaderMode, ReadableStreamType, ReadableWritablePair, ReferrerPolicy, Request, RequestCache, RequestCredentials, RequestInit, RequestMode, Response, ServiceWorkerGlobalScope, StreamPipeOptions, TransformStream, TransformStreamDefaultController, Transformer, UnderlyingSink, UnderlyingSource, Window, WorkerGlobalScope, WritableStream, WritableStreamDefaultController, WritableStreamDefaultWriter, console, default, std | MIT OR Apache-2.0 |
| weezl | 0.1.12 | alloc, default, std | MIT OR Apache-2.0 |
| wide | 0.7.33 | std | Zlib OR Apache-2.0 OR MIT |
| zerocopy | 0.8.62 | simd | BSD-2-Clause OR Apache-2.0 OR MIT |
| zerocopy-derive | 0.8.62 | none | BSD-2-Clause OR Apache-2.0 OR MIT |
| zlib-rs | 0.6.8 | rust-allocator, std | Zlib |

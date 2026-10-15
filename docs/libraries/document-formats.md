# Document-format libraries

The application's [Word package reader](../application/document-formats.md#word-package-reader)
and [presentation attachment extraction](../application/browser-attachments.md#loading)
share one package reader using these existing lockfile packages on native and browser targets:

| Package | Pin | Direct features | License choice |
|---|---|---|---|
| zip | 6.0.0 | defaults off; `deflate-flate2` | MIT |
| indexmap | 2.14.2 | defaults (`std`); ordered package-name identity | MIT |
| roxmltree | 0.21.1 | defaults off; `std`, `positions` | MIT |
| url | 2.5.8 | defaults | MIT |
| ryu-js | 1.0.3 | existing fixed-place number formatter | Apache-2.0 |

The resolved archive closure on both targets is crc32fast 1.5.2, cfg-if 1.0.5,
flate2 1.1.10, miniz_oxide 0.9.1, adler2 2.0.1, simd-adler32 0.3.10,
indexmap 2.14.2, equivalent 1.0.2, hashbrown 0.17.1, allocator-api2 0.2.21,
foldhash 0.2.0, serde_core 1.0.229 and memchr 2.8.3. Choose MIT for their
MIT/Apache-2.0 alternatives. The existing image/png dependency enables flate2's
Rust backend on both targets; package reading itself adds no zlib or zlib-rs
feature. The PDF engine below enables zlib-rs through its image decoder. roxmltree
reuses memchr 2.8.3 and borrows its input rather than requiring a second XML tree.

URL resolution retains form_urlencoded 1.2.2, percent-encoding 2.3.2, idna 1.1.0,
idna_adapter 1.2.2 and ICU 2.3's normalization, properties, collection, locale
and provider data. Their existing supporting crates include displaydoc,
proc-macro2, quote, syn, unicode-ident, potential_utf, writeable, zerovec, yoke,
yoke-derive, stable_deref_trait, utf8_iter, litemap, tinystr, zerofrom,
zerofrom-derive, zerotrie, zerovec-derive, smallvec, utf16_iter and write16.
Retain the MIT/Apache-2.0 and Unicode-3.0 license notices for this closure.
Presentation extraction adds only the application's direct indexmap edge,
not new resolved packages.

The XML adapter excludes namespace declarations from ordinary attributes,
concatenates descendant text and rejects DOCTYPE using the parser's native
policy. It retains source text separately from borrowed views. It adds no XML
schema validator, network resolver, renderer or archive-writing interface.
Presentation extraction reuses this parser for matched-run entity decoding, not
whole-slide admission; literal carriage returns are protected before decoding.

Source attribution is in the [third-party notices](../../THIRD-PARTY-NOTICES.md#word-package-reading).

## PDF processing

The application uses `hayro =0.8.0` and `hayro-interpret =0.8.0` with default
embedded fonts/CMaps and native SIMD. Text interpretation uses the glyph device;
previews reuse `render_into` with f64 transforms and the native pixmap PNG encoder.
No separate extraction or PNG library is introduced. The native engine's text
segmentation, Type3 text, bidirectional ordering and rasterization are retained.
The application reads inherited page-box numbers through the object API as f64
for thumbnail dimensions; it does not parse PDF syntax separately.

The resolved dependency closure is listed below. Choose MIT or Apache-2.0 where
available and preserve BSD-3-Clause, Zlib and Unicode-3.0 obligations. Embedded
PDFium/Foxit fonts retain BSD-3-Clause notices; the CGATS color profile is CC0-1.0.
The controlled PDF corpus retains its Apache-2.0 collection attribution.
See [third-party notices](../../THIRD-PARTY-NOTICES.md#pdf-processing).

| Package | Version | Declared license |
|---|---|---|
| adler2 | 2.0.1 | 0BSD OR MIT OR Apache-2.0 |
| alloc-no-stdlib | 2.0.4 | BSD-3-Clause |
| alloc-stdlib | 0.2.4 | BSD-3-Clause |
| arrayvec | 0.7.8 | MIT OR Apache-2.0 |
| autocfg | 1.5.1 | Apache-2.0 OR MIT |
| bitflags | 2.13.2 | MIT OR Apache-2.0 |
| brotli-decompressor | 5.0.3 | BSD-3-Clause/MIT |
| brotli | 8.0.4 | BSD-3-Clause AND MIT |
| bytemuck | 1.25.2 | Zlib OR Apache-2.0 OR MIT |
| bytemuck_derive | 1.12.1 | Zlib OR Apache-2.0 OR MIT |
| byteorder-lite | 0.1.0 | Unlicense OR MIT |
| cfg-if | 1.0.5 | MIT OR Apache-2.0 |
| color | 0.3.3 | Apache-2.0 OR MIT |
| crc32fast | 1.5.2 | MIT OR Apache-2.0 |
| euclid | 0.22.14 | MIT OR Apache-2.0 |
| fastrand | 2.5.0 | Apache-2.0 OR MIT |
| fdeflate | 0.3.7 | MIT OR Apache-2.0 |
| fearless_simd | 0.7.0 | Apache-2.0 OR MIT |
| fearless_simd | 1.1.0 | Apache-2.0 OR MIT |
| flate2 | 1.1.10 | MIT OR Apache-2.0 |
| foldhash | 0.2.0 | Zlib |
| font-types | 0.12.6 | MIT OR Apache-2.0 |
| glifo | 0.4.0 | Apache-2.0 OR MIT |
| guillotiere | 0.7.0 | MIT/Apache-2.0 |
| hashbrown | 0.17.1 | MIT OR Apache-2.0 |
| hayro-ccitt | 0.4.0 | Apache-2.0 OR MIT |
| hayro-cmap | 0.1.0 | Apache-2.0 OR MIT |
| hayro-interpret | 0.8.0 | Apache-2.0 OR MIT |
| hayro-jbig2 | 0.3.1 | Apache-2.0 OR MIT |
| hayro-jpeg2000 | 0.4.1 | Apache-2.0 OR MIT |
| hayro-postscript | 0.1.0 | Apache-2.0 OR MIT |
| hayro-syntax | 0.8.0 | Apache-2.0 OR MIT |
| hayro | 0.8.0 | Apache-2.0 OR MIT |
| image | 0.25.10 | MIT OR Apache-2.0 |
| kurbo | 0.13.1 | Apache-2.0 OR MIT |
| linebender_resource_handle | 0.1.1 | Apache-2.0 OR MIT |
| log | 0.4.34 | MIT OR Apache-2.0 |
| memchr | 2.8.3 | Unlicense OR MIT |
| miniz_oxide | 0.8.9 | MIT OR Zlib OR Apache-2.0 |
| miniz_oxide | 0.9.1 | MIT OR Zlib OR Apache-2.0 |
| moxcms | 0.8.1 | BSD-3-Clause OR Apache-2.0 |
| num-traits | 0.2.19 | MIT OR Apache-2.0 |
| once_cell | 1.21.4 | MIT OR Apache-2.0 |
| peniko | 0.6.1 | Apache-2.0 OR MIT |
| phf | 0.13.1 | MIT |
| phf_generator | 0.13.1 | MIT |
| phf_macros | 0.13.1 | MIT |
| phf_shared | 0.13.1 | MIT |
| pic-scale | 0.7.12 | BSD-3-Clause OR Apache-2.0 |
| png | 0.18.1 | MIT OR Apache-2.0 |
| polycool | 0.4.0 | MIT OR Apache-2.0 |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 |
| pxfm | 0.1.30 | BSD-3-Clause OR Apache-2.0 |
| quote | 1.0.47 | MIT OR Apache-2.0 |
| read-fonts | 0.41.0 | MIT OR Apache-2.0 |
| read-fonts | 0.43.3 | MIT OR Apache-2.0 |
| rustc-hash | 2.1.3 | Apache-2.0 OR MIT |
| simd-adler32 | 0.3.10 | MIT |
| siphasher | 1.0.4 | MIT OR Apache-2.0 |
| skrifa | 0.44.0 | MIT OR Apache-2.0 |
| skrifa | 0.46.2 | MIT OR Apache-2.0 |
| smallvec | 1.16.2 | MIT OR Apache-2.0 |
| stable_deref_trait | 1.2.1 | MIT OR Apache-2.0 |
| syn | 2.0.119 | MIT OR Apache-2.0 |
| syn | 3.0.6 | MIT OR Apache-2.0 |
| synstructure | 0.14.0 | MIT |
| thiserror-impl | 2.0.21 | MIT OR Apache-2.0 |
| thiserror | 2.0.21 | MIT OR Apache-2.0 |
| unicode-ident | 1.0.24 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| vello_common | 0.3.0 | Apache-2.0 OR MIT |
| vello_cpu | 0.3.0 | Apache-2.0 OR MIT |
| yoke-derive | 0.8.4 | Unicode-3.0 |
| yoke | 0.8.3 | Unicode-3.0 |
| zerofrom-derive | 0.1.8 | Unicode-3.0 |
| zerofrom | 0.1.8 | Unicode-3.0 |
| zlib-rs | 0.6.8 | Zlib |
| zune-core | 0.5.3 | MIT OR Apache-2.0 OR Zlib |
| zune-jpeg | 0.5.15 | MIT OR Apache-2.0 OR Zlib |

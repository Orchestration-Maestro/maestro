# Document-format libraries

The application's [Word package reader](../application/document-formats.md#word-package-reader)
uses these existing lockfile packages on native and browser targets:

| Package | Pin | Direct features | License choice |
|---|---|---|---|
| zip | 6.0.0 | defaults off; `deflate-flate2` | MIT |
| roxmltree | 0.21.1 | defaults off; `std`, `positions` | MIT |
| url | 2.5.8 | defaults | MIT |
| ryu-js | 1.0.3 | existing fixed-place number formatter | Apache-2.0 |

The resolved archive closure on both targets is crc32fast 1.5.2, cfg-if 1.0.5,
flate2 1.1.10, miniz_oxide 0.9.1, adler2 2.0.1, simd-adler32 0.3.10,
indexmap 2.14.2, equivalent 1.0.2, hashbrown 0.17.1, allocator-api2 0.2.21,
foldhash 0.2.0, serde_core 1.0.229 and memchr 2.8.3. Choose MIT for their
MIT/Apache-2.0 alternatives. The existing image/png dependency enables flate2's
Rust backend on both targets; no zlib or zlib-rs feature is added. roxmltree
reuses memchr 2.8.3 and borrows its input rather than requiring a second XML tree.

URL resolution retains form_urlencoded 1.2.2, percent-encoding 2.3.2, idna 1.1.0,
idna_adapter 1.2.2 and ICU 2.3's normalization, properties, collection, locale
and provider data. Their existing supporting crates include displaydoc,
proc-macro2, quote, syn, unicode-ident, potential_utf, writeable, zerovec, yoke,
yoke-derive, stable_deref_trait, utf8_iter, litemap, tinystr, zerofrom,
zerofrom-derive, zerotrie, zerovec-derive, smallvec, utf16_iter and write16.
Retain the MIT/Apache-2.0 and Unicode-3.0 license notices for this closure.
The lockfile update adds only the application's three direct dependency edges,
not new resolved packages.

The XML adapter excludes namespace declarations from ordinary attributes,
concatenates descendant text and rejects DOCTYPE using the parser's native
policy. It retains source text separately from borrowed views. It adds no XML
schema validator, network resolver, renderer or archive-writing interface.

Source attribution is in the [third-party notices](../../THIRD-PARTY-NOTICES.md#word-package-reading).

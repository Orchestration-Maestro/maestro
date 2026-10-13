# Browser utility libraries

The `maestro-web` utility library has exactly two internal dependencies:
`maestro-app` supplies existing numeric records and markup escaping;
`maestro-theme` supplies caller-resolved presentation. It introduces no runtime,
font bundle or application launcher.

| Library | Pin | Features used | License choice |
|---|---|---|---|
| egui | 0.36.2 | defaults off; color/image types | MIT |
| usvg | 0.48.1 | defaults off; SVG parsing; roxmltree re-export | MIT |
| resvg | 0.48.1 | defaults off; native rendering; tiny-skia re-export | MIT |
| indexmap | 2.14.2 | serde, default/std | MIT |
| ryu-js | 1.0.3 | workspace number formatting | Apache-2.0 |
| num-traits | 0.2.19 | workspace checked canvas-size conversion | MIT |
| serde | 1.0.229 | workspace derive | MIT |
| serde_json | 1.0.151 | workspace preserve_order, float_roundtrip | MIT |
| wasm-bindgen | 0.2.129 | wasm-only, default/std | MIT |
| js-sys | 0.3.106 | wasm-only, default/std | MIT |
| web-sys | 0.3.106 | wasm-only browser platform operations | MIT |

The web-sys feature set is Window, Navigator, Storage, Location, console,
Document, Node, Element and SvgElement. Storage and reload errors remain fallible. Legacy locale access uses the platform property
adapter only on the fallback branch. The real library is built for WebAssembly,
not replaced with a test-only host.

Native rendering parses serialized SVG; browser rendering creates SVG DOM
nodes directly. They share root defaults, size classes and numeric attribute
formatting. Tests inspect real XML literals and native raster dimensions,
alpha and supplied colors; controlled language hosts cover selection, lazy reads
and effect order.
They do not claim browser attachment, computed CSS or browser pixel identity.

The workspace lock retains its existing smallvec 1.16.2, syn 3.0.6,
unicode-ident 1.0.24 and uuid 1.27.0 resolutions rather than updating them to
the isolated qualification probe's newer patch versions. Indexmap's resolved
features include serde. The direct library pins above are unchanged.

Resolved dependencies include Unicode-3.0 for Unicode data, BSD-2-Clause for
arrayref, BSD-3-Clause for tiny-skia/tiny-skia-path, Zlib for foldhash/zlib-rs and
MIT or Apache-2.0 choices where available. The self_cell and ryu-js alternatives
are used under Apache-2.0; memchr is used under MIT. No JavaScript implementation
or runtime dependency is bundled.

Lucide 0.544.0 icon geometry uses ISC with Feather MIT portions. Browser
translation data uses MIT. Their full texts are retained in
[third-party notices](../../THIRD-PARTY-NOTICES.md).

See [browser utility use](../browser/frontend.md) for the delivered API boundary.

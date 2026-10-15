# Browser utility libraries

The `maestro-web` utility library has exactly two internal dependencies:
`maestro-app` supplies existing numeric records, markup escaping and attachment policy;
`maestro-theme` supplies caller-resolved presentation. It introduces no runtime or application launcher. Production installs no default
font bundle; native tests enable the dev-only egui default fonts.

| Library | Pin | Features used | License choice |
|---|---|---|---|
| egui | 0.36.2 | production defaults off; drawing, controls and color/image types; dev-only default_fonts | MIT |
| usvg | 0.48.1 | defaults off; SVG parsing; roxmltree re-export | MIT |
| resvg | 0.48.1 | defaults off; native rendering; tiny-skia re-export | MIT |
| base64 | 0.23.1 | defaults off/std; offline export payload decoding | MIT |
| url | 2.5.8 | default; form-urlencoded deep-link parsing | MIT |
| indexmap | 2.14.2 | serde, default/std | MIT |
| ryu-js | 1.0.3 | workspace number formatting | Apache-2.0 |
| num-traits | 0.2.19 | workspace checked canvas-size conversion | MIT |
| serde | 1.0.229 | workspace derive; application/agent rc for retained records | MIT |
| serde_json | 1.0.151 | workspace preserve_order, float_roundtrip | MIT |
| wasm-bindgen | 0.2.129 | wasm-only, default/std | MIT |
| js-sys | 0.3.106 | wasm-only, default/std | MIT |
| web-sys | 0.3.106 | public native event/element types; wasm-only platform effects | MIT |
| wasm-bindgen-futures | 0.4.79 | wasm-only, default/std request promises and local future dispatch | MIT |

The web-sys feature set is Window, Navigator, Storage, Location, console,
Document, Node, Element, SvgElement, HtmlElement, HtmlAnchorElement, Blob,
BlobPropertyBag, Url, MediaQueryList, HtmlCanvasElement, DomRect, DomException,
DomStringList, IdbDatabase, IdbFactory, IdbIndex, IdbIndexParameters, IdbObjectStore,
IdbObjectStoreParameters, IdbOpenDbRequest, IdbRequest, IdbTransaction,
IdbTransactionMode, HtmlInputElement, Event, EventTarget and KeyboardEvent.
Storage and reload errors remain fallible. Legacy locale access uses the platform property
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

See [browser utility use](../browser/frontend.md) and
[browser-native record storage](../browser/storage.md) for the delivered API boundaries.
IndexedDB uses the browser platform and the existing application configuration
records, with no new codec, executor or JavaScript storage implementation. The
added request-future declaration and platform features change no locked
third-party package version.

Application attachment loading reuses already-landed native libraries: serde
1.0.229 (derive, MIT), base64 0.23.1 (defaults off/std, MIT), rand 0.10.3
(defaults, MIT), ryu-js 1.0.3 (Apache-2.0) and futures-util 0.3.34
(defaults off/std/sink, MIT). The existing model Fetch port owns HTTP transport;
there is no second client or document parser dependency. The application uses
the already-allowed agent dependency for standard-message projection.

Native tests enable `egui/default_fonts` through a dev-dependency, resolving
`epaint_default_fonts =0.36.2` (MIT, OFL-1.1 and Ubuntu-font-1.0). The exact-version
licence exception is in `.github/ci.toml`. The egui traversal exclusion in
`.config/hakari.toml` keeps this feature out of production feature unification.
Normal/build dependency trees remain font-free on the five configured native
targets and WebAssembly; the development tree still enables real fallback fonts.
These fonts do not establish brand-face or real-browser glyph qualification.

Application session storage reuses chrono 0.4.45 (defaults off/std, MIT or
Apache-2.0) for UTC millisecond text, the text library's native collation feature
(ICU collator 2.3.1 and locale core 2.3.0, Unicode-3.0), and js-sys 0.3.106
(default/std, MIT) on browser targets for default-locale comparison. These are
existing locked libraries, with no new runtime or storage adapter dependency.

The [offline document reader](../browser/session-viewer.md) directly reuses the
locked base64 and URL libraries already below the application. URL form decoding
uses form_urlencoded 1.2.2 and percent-encoding 2.3.2; its existing IDNA/ICU closure
adds no new package versions. Unicode data retains its Unicode-3.0 requirement.

Sandbox routing reuses the locked browser bindings with the `HtmlIFrameElement`,
`MessageEvent` and `EventTarget` web-sys features. Raw values use native js-sys
JSON/string operations; listener work uses wasm-bindgen-futures. No package
version, executor or native dependency is added. The embedded messaging scripts
are host-required assets, not a JavaScript frontend implementation. See
[sandbox messaging](../browser/sandbox.md).

Owned native reading surfaces additionally enable `web-sys/ResizeObserver` on
that same locked version. This feature adds no package dependency. The existing
native event-subscription owner retains its scroll callback; the observer shares
that callable and disconnects before it is released. No font or browser-runner
package is installed for reading content.

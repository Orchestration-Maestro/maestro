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
| base64 | 0.23.1 | defaults off/std; offline export and generic-artifact payload decoding | MIT |
| url | 2.5.8 | default; form-urlencoded deep-link parsing | MIT |
| percent-encoding | 2.3.2 | default; opaque query-key encoding | MIT |
| indexmap | 2.14.2 | serde, default/std | MIT |
| ryu-js | 1.0.3 | workspace number formatting | Apache-2.0 |
| num-traits | 0.2.19 | workspace checked canvas-size conversion | MIT |
| serde | 1.0.229 | workspace derive; application/agent rc for retained records | MIT |
| serde_json | 1.0.151 | workspace preserve_order, float_roundtrip | MIT |
| wasm-bindgen | 0.2.129 | wasm-only, default/std | MIT |
| js-sys | 0.3.106 | wasm-only, default/std | MIT |
| web-sys | 0.3.106 | public native event/element types; wasm-only platform effects | MIT |
| wasm-bindgen-futures | 0.4.79 | wasm-only, default/std request promises and local future dispatch | MIT |
| custom-element | 0.1.5 | wasm-only; autonomous light-DOM element classes and their shipped 43-line subclass shim | MIT |

The web-sys feature set is Window, Navigator, Storage, Location, console,
Document, Node, Element, SvgElement, HtmlElement, HtmlAnchorElement, Blob,
BlobPropertyBag, Url, MediaQueryList, HtmlCanvasElement, DomRect, DomException,
DomStringList, IdbDatabase, IdbFactory, IdbIndex, IdbIndexParameters, IdbObjectStore,
IdbObjectStoreParameters, IdbOpenDbRequest, IdbRequest, IdbTransaction,
IdbTransactionMode, IdbKeyRange, IdbCursor, IdbCursorWithValue, IdbCursorDirection,
StorageManager, StorageEstimate, HtmlInputElement, HtmlTextAreaElement, Event, EventTarget, KeyboardEvent,
CustomElementRegistry, HtmlButtonElement and Response.
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

custom-element 0.1.5 is the browser element bridge: it defines classes whose
lifecycle callbacks reach Rust state, through its fixed `createCustomElement.js`
shim (43 lines), which binding generation emits beside the module. It depends only
on the locked wasm-bindgen, js-sys and web-sys and adds no other package to the
workspace lock. Its whole browser closure is 25 packages under MIT or Apache-2.0
where a choice exists, plus Unicode-3.0 for unicode-ident. The crate is
dual-licensed MIT OR Apache-2.0 and used under MIT. The two runtime-module test
fixtures are separate crates with their own lockfiles, checked by shared CI's
locked-manifest check. See [third-party notices](../../THIRD-PARTY-NOTICES.md#browser-element-bridge).

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
Native application tests explicitly enable futures-util's `async-await` feature
for the prompt-token example's `poll!` assertions, rather than relying on
platform-dependent transitive feature unification. This adds no package.

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

The offline header reuses these same bindings for literal metadata, identified
canvas-control slots and native date localization. Native Date construction and
zero-argument locale formatting use existing js-sys reflection. Geometry readback
uses a typed catchable binding without realm-local rectangle casts, so native
failures reach transaction cleanup.
Slot sizing, optional-style restoration and fractional-scroll restoration share
one owner for measurement and publication. No package, binding feature, font or
browser-runner dependency is added.
Generic artifact views reuse the existing reading registry, native SVG bindings,
canvas ghost control and original-byte download lifecycle. The byte control is
adapted under MIT; its copyright and permission text are retained in
[third-party notices](../../THIRD-PARTY-NOTICES.md). Native artifact panel timestamps
reuse chrono 0.4.45 (defaults off/std, MIT or Apache-2.0) on browser targets to name the existing
application clock return type. Composition tests keep the same development
declaration. No package version or native production edge is added.

Retained sandbox loading additionally enables `web-sys/DomParser` and
`web-sys/SupportedType` on the same locked version, with no added package or
licence. Native HTML parsing owns recovery and validation; iframe sandbox/style
attributes need no additional binding features. Document preparation embeds only
the runtime and navigation programs required by the sandbox, not a JavaScript
frontend. See [retained HTML loading](../browser/sandbox.md#retained-html-loading).

Message links directly declare the already-locked percent-encoding library and
add only `web-sys/HtmlTextAreaElement`. Its implied Element, EventTarget,
HtmlElement and Node features were already enabled. Clipboard discovery, Promise
settlement and fallback invocation reuse existing reflection and bindings; no
package version, license closure or clipboard runtime is added.

Hidden sandbox execution directly uses Tokio `=1.53.3`, defaults off with only
`sync`, for an owned one-shot result channel. Tokio is MIT; its already-locked
`pin-project-lite =0.2.17` closure is MIT OR Apache-2.0. No package version,
runtime, executor or timer implementation is added. The adapter reuses the native
browser timer, cancellation and event-subscription owners. See
[hidden execution](../browser/sandbox.md#hidden-javascript-execution).

Transcript Markdown directly declares `katex-rs =0.3.0` as `katex` (MIT, defaults off) for HTML
math output and the already-locked `regress =0.12.0` (`prohibit-unsafe`) for its
tag-escaping expressions. Its closure resolves bon, bon-macros, darling,
darling_core, darling_macro, ident_case, the phf family, rapidhash and strsim
beside packages the workspace already locks. The matching KaTeX 0.18.5 stylesheet
and its twenty `woff2` fonts are embedded and inserted once per document as data;
their MIT notice is in [third-party notices](../../THIRD-PARTY-NOTICES.md#math-stylesheet-and-fonts).
A private typed adapter supplies missing bare-radical padding, preserves display
style when changing explicit size and script style when repeating it, and routes
builtin macro errors to inline error spans.
Malformed environment names retain the delimiter-spelled fallback. The registry
package stays unchanged; there is no fork, formula preprocessing or HTML rewriting.
Internal markup and error-tooltip wording may differ without changing the rendered
formula or its error branch. The adapter-derived source shares the math MIT notice.
Parsing is the shared [Markdown owner](../markdown.md), reached through the
application edge; no browser binding feature is added.

## Isolated author fixture

The [browser author fixture](../browser-build.md) is a separate Cargo workspace,
not a new `maestro-web` runtime dependency. It builds its page from elements with
wasm-bindgen `=0.2.129` and web-sys `=0.3.106` (MIT OR Apache-2.0); web-sys
selects Document, Element, HtmlElement, Node and Window. Native preparation invokes
the existing `maestro-theme` export example as a separate locked root build; tooling
reuses already-locked serde_json `=1.0.151` (MIT) for generated configuration. The
fixture has no native path or build-dependency edge. The shared whole font assets
have OFL-1.1 notices, and the fixture stylesheet registers them.


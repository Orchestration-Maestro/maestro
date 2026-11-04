# Browser sandbox runtime

## Problem statement

Maestro's browser sandbox embeds ten handwritten JavaScript runtime files. That implementation conflicts with its Rust-only requirement. Artifact authors must retain their existing helpers, messages, execution order, isolated realm and offline downloads when Maestro replaces the runtime. **Confidence: proven requirement. Evidence: C01, C02, C08-C11.**

## Solution

Build Maestro's sandbox runtime as Rust for the browser, using the existing locked browser bindings. Install it in the isolated frame before the authored scripts that depend on it. Supplied JavaScript remains executable content; compiler-generated binding glue remains generated output. Keep one provider interface, frame lifecycle, router and result collector. **Confidence: proven requirement; reconstruction likely; startup qualification unknown. Evidence: C01-C02, C07-C11.**

The implementation is blocked on a measured bootstrap, public-API and content-security-policy receipt. A deferred module placed beside active HTML does not establish installation order. The [bootstrap qualification receipt](#bootstrap-qualification-receipt) selects parser-blocking generated startup and lists the decisions still open before dispatch. Five required rows are not qualified yet. **Confidence: measured for the prototype; open decisions and five unqualified rows listed. Evidence: C08-C09, C12; the receipt below.**

## User stories

Each story is a requirement, not a passing implementation claim.

### Artifact programs

1. As an artifact author, I want the first classic script after the runtime insertion point to see its selected helpers, so that startup does not race runtime initialization. **Proven: C08-C09, C12.**
2. As an artifact author, I want artifact reads, writes and failures to retain their online and offline meanings, so that stored files behave consistently. **Proven: C03.**
3. As an artifact author, I want synchronous attachment reads and byte-preserving file returns, so that binary data is not reinterpreted as text. **Proven: C04, C06.**
4. As a downloaded-artifact user, I want one HTML file that works without networking or a parent service, so that I can open it independently. **Proven: C03-C04, C06, C08, C11.**

### Execution and embedding

1. As a tool user, I want console output, asynchronous results and errors collected before settlement, so that successful and failed runs retain their current output. **Proven: C05, C09-C10.**
2. As a provider author, I want ordered data/runtime entries and supplied runtime content, so that custom capabilities remain available without making built-ins authored script strings. **Proven: C02, C08.**
3. As an embedding host, I want the current-frame sender checks and one-shot URL handshake preserved, so that unrelated or replaced frames cannot act as the registered frame. **Proven: C07, C10.**
4. As an embedding host, I want exact CSP requirements and refusal behavior documented, so that deploying Rust does not silently widen script authority. **Proven requirement; replacement policy unknown: C01, C12.**

## Implementation decisions

### S01: authored code and existing owners

**Confidence: proven requirement; placement likely. Evidence: C01-C02, C07-C11, C13-C14.**

All Maestro-authored provider, bridge, navigation, envelope, completion and bootstrap logic becomes Rust. Delete the ten handwritten runtime assets and their includes after the replacement passes. Moving their programs into Rust literals, a handwritten emitter, renamed assets or a script fallback does not satisfy this requirement. HTML markup, inert data, supplied programs and generated bindings are distinct from authored runtime logic.

Application owners retain provider data, descriptions, message handling, artifact commands/history and captured results. The browser frontend owns in-realm installation and browser effects. Repository tooling prepares prebuilt runtime assets through the existing author-build path. Add no crate, reverse application-to-web edge, runtime compiler at install/use, second router, store, sandbox, worker framework or mutable global bundle registry.

Use the existing exact pins: web-sys/js-sys 0.3.106, wasm-bindgen 0.2.129 and wasm-bindgen-futures 0.4.79. This contract approves no new dependency or feature. The existing event-subscription owner retains capture selection when navigation needs it; existing callers retain their current default. Listener removal uses the same capture value as registration.

### S02: provider selection and installation order

**Confidence: proven behavior; typed representation likely. Evidence: C02, C08-C09.**

Keep `SandboxRuntimeProvider::get_runtime` as the provider operation. Its planned return is `Result<Runtime, DiagnosticErrorInfo>`, where the application-owned selection is `Builtin(BuiltinRuntime)`, `Content(String)` or `Empty`. The four built-in selections are Artifacts, Attachments, Console and FileDownload. `Content` preserves supplied user/extension function content receiving the sandbox identifier; Maestro's built-ins never use it. `Empty` replaces the existing no-op content provider. Keep the value projections, descriptions, data, message and lifecycle hooks unchanged.

Preparation first collects every provider's data in entry order, then every runtime selection in entry order, then serializes the merged globals with native browser operations. Later values replace earlier keys. Preserve native key enumeration, property assignment and serialization failures, including undefined, cyclic values and BigInt. Do not replace browser values with a JSON-only transport.

In the frame, install in this order: sandbox identifier, merged globals, optional bridge and completion registration, each provider entry, optional navigation, then authored programs. Global/provider overwrites remain observable. Repeated provider entries run repeatedly. Load and execute prepend a fresh console provider; public preparation does not. Standalone preparation omits the bridge and navigation, not selected provider helpers.

### S03: artifact helpers

**Confidence: proven in source. Evidence: C03.**

Install async `listArtifacts`, `getArtifact`, `createOrUpdateArtifact` and `deleteArtifact`. Each reads live globals at call time. Online requests retain `type: artifact-operation` and actions `list`, `get`, `createOrUpdate`, `delete`; retain filename/content and the optional `mimeType`. A non-success response rejects; reads return the raw result and successful mutations resolve without a value.

Offline listing uses native own-key order. Offline reads accept own properties, including empty-string content, and reject missing or inherited entries. Parse only filenames ending in lowercase `.json`; malformed JSON rejects. Serialize non-string writes with native two-space JSON regardless of filename. Offline mutations reject. Keep the existing corrected empty-file lookup; introduce no cache or second artifact store.

### S04: attachment helpers

**Confidence: proven in source. Evidence: C04.**

Install synchronous `listAttachments`, `readTextAttachment` and `readBinaryAttachment`. List exposes id, fileName, mimeType and size only. Read the live attachment global on each call and select the first strict matching id, including repeated ids. Truthy extracted text takes precedence; empty extracted text falls back to base64 content.

Text fallback uses native `atob` binary-string behavior, not UTF-8 decoding. Binary reads produce `Uint8Array` values with every byte unchanged. Missing ids and invalid base64 retain their synchronous failure categories.

### S05: console and terminal messages

**Confidence: proven in source. Evidence: C05.**

Keep the first bound original log/error/warn/info methods in `__originalConsole`; repeated installation uses those originals. Wrappers accept all arguments. Format each object with native compact JSON and other values with native String, falling back to native String on serialization failure; join the results with spaces. Preserve zero arguments, native formatting failures and raw argument handles. Call the original console before forwarding `type: console`, method, text and args.

Track pending send promises with their rejection handler. Register the pending-send join through `onCompleted` only when it exists. Error and unhandled-rejection listeners retain the latest message/stack without adding another console entry.

`complete(error, returnValue)` returns a promise and admits only its first call, before asynchronous work. A truthy explicit error wins over the recorded error. With messaging installed, send `execution-error` plus error, or `execution-complete` plus the raw return value. Preserve missing, undefined and null distinctions at the current boundaries. With no bridge, send nothing. Completion itself does not run completion callbacks and adds no terminal acknowledgement.

### S06: bridge globals, correlation and transport

**Confidence: proven in source. Evidence: C07.**

Both contexts reset the public mutable `__completionCallbacks` array and install `onCompleted` as an append to its current value. User edits or replacement of that public array remain visible; a hidden Rust callback list is not equivalent.

Iframe `sendRuntimeMessage` returns a promise. Install its reply listener before posting. Copy supplied enumerable fields, then authoritative sandboxId and request messageId. Distinct concurrent requests correlate independently. A reply must have `type: runtime-response` and the matching messageId; successful replies resolve the raw response, failed replies reject, and the production deadline is 30,000 ms. Release the request listener after reply or deadline; propagate posting failures. Request identifiers correlate requests, not authenticate senders.

The child reply listener currently checks type/id, not source or origin. Keep that policy separate from the host's current-frame admission checks. Do not silently add a child-origin rule or weaken the host gate.

User-script context instead copies supplied fields followed by authoritative sandboxId and awaits `chrome.runtime.sendMessage`. It retains native fulfillment/rejection and does not acquire iframe correlation listeners or the iframe deadline.

### S07: navigation

**Confidence: proven in source. Evidence: C09-C10, C12.**

Capture click events and resolve the nearest anchor through native hyperlink state. Preserve effective HTML and SVG HTTP(S) addresses, including namespaced, animated, relative and hash-bearing links. For a selected HTTP(S) address, prevent default, stop propagation and post `open-external-url` with url to the parent using `*`. Other click schemes pass through.

Capture form submission and route a truthy effective action without adding the link scheme filter. Attempt the location accessor interception and tolerate native refusal. Standalone documents install none of these interceptors. The parent keeps its current-source check and extension-tabs versus `_blank` opening behavior.

### S08: document preparation and asynchronous user content

**Confidence: proven behavior; replacement mechanism unknown. Evidence: C08-C09, C11-C12.**

Preserve authored HTML insertion after the first case-insensitive head prefix, otherwise html prefix, otherwise before the source. Preserve existing prefix matching, native HTML recovery and the font-size reset. Do not infer HTML mode from JavaScript string literals. Keep quoted global names and literal closing-script text, CR, Unicode, NUL and marker-looking inputs safe; preserve their existing parser-visible results.

User programs retain await, return, thrown values, dynamic imports, `import.meta.url` and relative-module resolution. Preserve the current parser/module/defer/DOMContentLoaded relationships and document base behavior. The replacement cannot assume that a synchronous Function or an ordinary module accepts an asynchronous function body with return. It cannot move the handwritten prefix/suffix into Rust strings.

Rust owns result and callback settlement. On success, invoke callbacks with true in registration order, join through native Promise.all, then await `complete(null, returnValue)`. On failure, invoke them with false and complete with the current message/stack projection. A synchronous callback throw stops that map walk; a returned rejection is observed after the map has invoked later entries. Promise.all is fail-fast, not all-settled. Log callback failure without replacing the user outcome. Preserve the surrounding failure path if the awaited successful completion itself rejects.

The HTML artifact completion signal remains after the entire authored source, including source following a closing html tag. Replacing it with a load event or moving it ahead of that source is not equivalent. Preserve its ordering relative to deferred/module scripts through browser witnesses.

### S09: lifecycle, sender checks and native values

**Confidence: proven in source. Evidence: C07-C11.**

Reuse the retained frame owner for loading, SDK execution, application hidden execution, contributions and downloads. Keep `allow-scripts allow-modals` without same-origin permission. Register before mounting; only the current frame may settle the supplied-URL ready/error handshake, once. Keep source validation for normal and bootstrap-error traffic; a guessed sandbox id or origin text is insufficient.

Retain provider-before-consumer dispatch, live provider entries and identity-deduplicated live consumers, response-field precedence, current reply destinations, cancellation, end-hook ordering, file retention and retryable cleanup. Preserve raw browser values without a JSON round trip, including byte-array handles and absent/null returns. The cooperative execution deadline stays 120,000 ms, separate from the bridge deadline. This change adds no hard interruption guarantee.

### S10: prebuilt bundle and public API receipt

**Confidence: likely reconstruction; final loader/API unknown. Evidence: C01-C02, C08, C11, C13.**

Prepare one matching runtime WebAssembly artifact and generated binding bundle through the existing build owners. A runtime-only browser entry beside the existing adapter is the candidate placement; it must not load the full chat application in the frame or create a recursive build dependency. Installation occurs in the isolated realm, never by lending the parent DOM's authority.

Standalone output embeds its matching runtime, binding glue and selected provider data in one HTML file. It must run with networking unavailable, without sibling files, parent traffic, a local service or consumer compilation. An application-relative wasm fetch does not satisfy this capability.

Before implementation dispatch, the bootstrap receipt must name the replacement for the public `generate_bridge_code` API, retaining the bridge's iframe and user-script roles and existing options/context meanings. It must state the exact bundle supply and constructor signatures, or prove existing signatures can stay. Update every affected caller and root export; add no compatibility facade that keeps authored runtime code alive. The typed provider selection above is planned; compile its final public spelling in the receipt before consumers change.

### S11: bootstrap and content-security-policy qualification

**Confidence: unknown replacement feasibility. Evidence: C08-C09, C12-C13.**

Compare parser-blocking generated startup with Rust-controlled inert-document staging. An asynchronously initialized module beside active user scripts is not sufficient. Qualify first-script helper access, delayed initialization, repeated providers, supplied runtime content, both loading paths, cancellation during startup and self-contained offline output.

Record the actual inherited parent policy and supplied-URL policy. Identify every required directive and resource for generated glue, WebAssembly compilation and user-program execution. Test hash admission and unmatched-script refusal, Trusted Types with an HTML-only default, in-frame Trusted Types none, opaque-origin module/CORS behavior and the selected data/blob/resource mechanism. `wasm-unsafe-eval` is not permission for JavaScript eval or Function.

Do not add same-origin permission, generic unsafe-eval, wildcard script/connect origins or a bypass policy. A necessary policy or observable behavior difference returns for an explicit decision before implementation continues. Changing boot implementation hashes is not permission to remove unmatched-script rejection.

Measure prototype and final release bytes separately. T01 measures the selected bootstrap path's size limit: the largest tested module that still installs before classic scripts under the measured CSP. Record the exact browser/toolchain, probe inputs, passing/failing sizes and failures next to an estimate of the complete optimized runtime, including its basis and uncertainty. If that estimate exceeds the measured limit, T01 reports the result and the implementation is not dispatched. An unestablished limit or unsupported estimate is an incomplete receipt, not a pass. No throwaway full runtime is required. The final acceptance reruns identical witnesses against the actual optimized all-provider bundle. A size-dependent failure returns for a decision, with no authored-script fallback or policy relaxation. The measured browser limit is qualification evidence, not an invented product limit.

## Bootstrap qualification receipt

This receipt records what the bootstrap qualification measured. It selects a loader and names the public API, but it does not deliver the production runtime: the ten handwritten runtime files and their envelopes are unchanged until the implementation issue lands. Five required rows have no witness yet and are marked not qualified under [open questions](#open-questions-and-required-receipts). Until they pass, this receipt does not unblock the implementation issue.

### Measurement setup

The pinned Chromium job ran Chromium 156.0.8078.4 (supplier revision 1248), wasm-bindgen CLI and library 0.2.129, js-sys and web-sys 0.3.106 and the repository's pinned Rust toolchain. The prototype is a Rust runtime bundle beside the existing browser-module fixtures, `crates/maestro-web/tests/fixtures/browser_modules/sandbox_runtime`. It has its own manifest and lock and uses no dependency or feature beyond the existing fixture set. The browser runner's package step builds it in release for `wasm32-unknown-unknown` and generates classic-script glue with `wasm-bindgen --target no-modules --no-modules-global __maestroSandboxRuntime --no-typescript`. Eight exports in the loading family exercise it, in `crates/maestro-web/tests/sandbox_bootstrap_browser.rs`. Each export logs `RECEIPT bootstrap` lines and asserts the rows below. The qualifying run is [41773572302](https://github.com/Orchestration-Maestro/maestro/actions/runs/41773572302) (`COMPLETE 277/277`). A failed-and-restored mutation run, [41773604662](https://github.com/Orchestration-Maestro/maestro/actions/runs/41773604662), turns all eight exports red (`COMPLETE 269/277`). It skips the last selection, completes before the joined callbacks and stops the compile search below the limit.

The prototype installs the sandbox identifier and merged data globals, then each runtime selection in entry order. Its one built-in is a live synchronous `listAttachments`. It also runs supplied providers, settles asynchronous programs and stages documents for the comparison path.

### Selected loader: parser-blocking generated startup

At the runtime insertion point, the prepared document carries, in order:

1. one inert carrier per supplied provider: `<template data-maestro-sandbox-provider><svg><script>` holding `(<function>)(<quoted sandbox id>);`, encoded as today;
2. one inert configuration block, `<script type="application/json" data-maestro-sandbox>`, holding the sandbox identifier, the merged globals and the ordered selections, with `<` written as `\u003c`;
3. the generated glue as a classic script (placement below);
4. one loading statement: `__maestroSandboxRuntime.initSync({module:Uint8Array.fromBase64("<module>")});`.

Synchronous instantiation runs the Rust start function before the parser reaches the next element. It assigns `sandboxId` and every global, then installs each selection in order. At a supplied provider's position it moves the carrier's parser-created script into the head, where it runs at once, then removes the carrier. Last it removes the configuration block and the loading script. JavaScript programs run from a hidden SVG module carrier holding `__maestroSandboxRuntime.run(async () => {\n<code>\n});`. Rust removes the carrier and awaits the program. It invokes `__completionCallbacks` in order with true or false, joins them through `Promise.all` and logs a failed join without replacing the outcome, then awaits `complete`.

Three generated call expressions remain as JavaScript text, none with runtime logic: the loading statement, the program envelope (an asynchronous body with `return` cannot be compiled without `Function` or `eval`) and the supplied-provider invocation, which keeps today's calling contract. Keeping them is a decision for the implementation issue; the measured alternative to the envelope is the generic eval permission this contract forbids.

The measured alternative, Rust-controlled staging, fails the Trusted Types row (see the staging comparison).

### Glue placement on `about:srcdoc`

The generated no-modules glue computes its own address as it loads: `new URL(document.currentScript.src, location.href)`. An inline script has an empty `src`. The `about:srcdoc` address has an opaque path, so the parse fails with `TypeError: Failed to construct 'URL': Invalid URL` and no helper installs. Blob and data addresses have opaque paths too. Documents with hierarchical addresses keep the glue inline: the supplied sandbox URL, and a downloaded file opened from disk. A srcdoc frame loads the same glue as a parser-blocking `<script src=… crossorigin>` from the application origin, followed by the inline loading statement. CORS mode keeps the glue's own errors readable in the opaque-origin frame; the serving host answers with `Access-Control-Allow-Origin`. Maestro never edits generated output. Serving the glue for srcdoc frames is decided: only the glue is served, and the module stays embedded inline, as measured.

### Size, compilation and reproducibility

| Measurement | Result |
| --- | --- |
| Main-thread synchronous compilation limit | 8,388,608 bytes compile. 8,388,609 bytes are refused: ``RangeError: WebAssembly.Compile is disallowed on the main thread, if the buffer size is larger than 8MB. Use WebAssembly.compile, compile on a worker thread, or use the flag `--enable-features=WebAssemblyUnlimitedSyncCompilation`.`` |
| Search method | Binary search between the bundle's own size and 64 MiB, padding the real module with one custom section |
| Largest module installed before classic scripts under the hash policy | 8,388,608 bytes, in an 11,185,940-byte srcdoc document; the next byte is refused with the same `RangeError` |
| Prototype bundle (release, no wasm-opt) | module 79,470 bytes (SHA-256 `cf9e5c912222ab1bb9dced6ef1cc5c8c35e1e801a9a619def10448c283a68bc9`); glue 23,644 bytes (SHA-256 `d2924055d8697b1a623f06aff4c2d95ff02ce5714d2b018b5efc5617a0fc8de4`); inline glue and loader 129,674 bytes |
| Reproducibility | In the qualifying run, two release builds, the second uncached in its own target directory, produced byte-identical glue and module. The planned generator's `--check` mode is the lasting gate |
| Full-runtime estimate | At most about 0.93 MB (79,470 × 11.7), about 11% of the limit |

The estimate scales the whole prototype module by the ratio of the runtime JavaScript it replaces. The prototype's behaviour covers 1,269 bytes, about 1.3 KB, against today's 14,827 bytes of handwritten runtime JavaScript, a factor of about 11.7. The covered bytes are the attachment listing (220 bytes of the attachments `runtime.js`), settlement (`repl-prefix.js`, 64, and `repl-suffix.js`, 779), the two boot programs (the head carrier script, 117, and the body carrier script, 54) and data installation (the fixed text of the `window.sandboxId = …;` statement, 21, and of one `window[…] = …;` global, 14). The boot programs and the data statements are Rust string templates, not `.js` files. Completion registration (the bridge `runtime.js`) is not covered: the witnesses assign `__completionCallbacks` themselves. Scaling the whole module overstates the result, since glue, allocator and binding support are fixed costs, and wasm-opt is not applied. A threefold error in Rust bytes per behaviour still gives about 2.8 MB, under the limit. The estimate does not exceed the measured limit, so size does not block implementation dispatch. The final acceptance reruns these witnesses against the actual optimized bundle.

### Policy matrix

| Path and policy | Observation |
| --- | --- |
| srcdoc, inherited `script-src 'self' <digests> 'wasm-unsafe-eval'` | Selections install in order before the first head and body classic scripts, at the real size and at the limit |
| srcdoc, digest of the loading statement missing | Loading refused without an exception; helpers absent; the carrier stays inert |
| srcdoc, digest of encoded carrier markup instead of decoded text | That provider is refused; the built-ins around it still install |
| srcdoc, inherited `require-trusted-types-for 'script'` with an HTML-only default policy | Installs unchanged; no Trusted Types sink is used |
| srcdoc, the frame document requires Trusted Types with no policy object | Installs unchanged |
| srcdoc, a provider turns Trusted Types to `none` inside the frame | Later selections and providers still install |
| srcdoc, `'wasm-unsafe-eval'` missing | `CompileError`: the module violates `script-src` because `'unsafe-eval'` is not allowed; helpers absent |
| srcdoc, inline glue | `TypeError: Failed to construct 'URL': Invalid URL`; helpers absent |
| Supplied URL, document policy `script-src <digests> 'wasm-unsafe-eval'` | Inline glue; installs in order before the first head and body classic scripts |
| Offline single file, `default-src 'none'; script-src <digests> 'wasm-unsafe-eval'` | Inline glue and embedded module install with every resource refused |
| Asynchronous programs, srcdoc, `script-src 'self' <digests> 'wasm-unsafe-eval'` | await, return, throw, `import.meta.url` equal to the base URL, relative dynamic import and ordered callbacks complete without `Function` or `eval` |

Today's frame needs no WebAssembly permission. Hosts pin four digests: runtime, program module, head boot and body remover. The selected loader changes that policy as follows:

- add `'wasm-unsafe-eval'` to the frame's script policy. Pages under a policy already need it for their own modules ([browser frontend](../browser/frontend.md)). It grants no JavaScript `eval` or `Function`;
- pin the loading statement's digest. It is fixed for one bundle, because the configuration moved to the inert data block. Pin each supplied provider invocation and the program module as today. The head boot and body remover digests disappear;
- on srcdoc, admit the served glue through the existing `'self'` source and serve it with CORS headers.

The change adds no same-origin permission, generic `unsafe-eval`, wildcard origin, `blob:` or `data:` source, and no policy bypass. Unmatched scripts are still refused.

### Staging comparison

The staging path initializes the same module asynchronously in a document that holds only the configuration and the loader. It then calls `document.open`, installs, and writes the authored document with `document.write`. Without Trusted Types it installs before the first classic script. Under the inherited `require-trusted-types-for 'script'` with an HTML-only default policy, the case today's runtime passes, the frame realm has no policy object. `document.write` is refused with `TypeError` and the authored document never runs. `document.open` also discards every window listener and adds a second load event. The parser-blocking loader passes that same row, so staging is not selected.

### Public API and callers

Planned spellings, compiled by the implementation issue before consumers change:

- Provider selection, application-owned: `SandboxRuntimeProvider::get_runtime(&self) -> Result<Runtime, DiagnosticErrorInfo>` with `enum Runtime { Builtin(BuiltinRuntime), Content(String), Empty }` and `enum BuiltinRuntime { Artifacts, Attachments, Console, FileDownload }`. The four built-in providers return `Builtin`. The contribution adapter (`crates/maestro-web/src/contributions/adapters.rs:265`) returns `Content`. Test providers in the app and web integration targets switch to the same return type.
- Bundle supply: a runtime-only entry, the `sandbox_runtime` cdylib example of `maestro-web`, built by a repository-tooling generator. The generator runs a release build and `wasm-bindgen --target no-modules`, then writes the glue and module as a checked-in generated bundle under `crates/maestro-web/src/components/sandbox/`. The library embeds them with `include_str!` and `include_bytes!`. A `--check` mode regenerates and compares them in CI. The example never reads the embedded bytes, so the build has no cycle. The measured reproducibility above is what makes `--check` stable.
- Constructors and defaults stay as they are: `SandboxIframe::new` (`html_artifact/body.rs:208`, `contributions/adapters.rs:222`), `BrowserSandboxHost`, the `JAVASCRIPT_REPL_TOOL` default (`tools/javascript_repl.rs:28`) and `prepare_html_document` (`browser/frame.rs:34`, `browser/execution.rs:273`, `html_artifact/header.rs:144`).
- Bridge: `RuntimeMessageBridge::generate_bridge_code` (`runtime_message_bridge.rs:48`, root export `lib.rs:90`) stops generating source. The iframe bridge becomes a configuration field the runtime installs. Its one production caller (`sandboxed_iframe/runtime.rs:21`), the documentation example and the two test callers (`tests/runtime_bridge.rs`, `tests/sandbox_exports.rs`) change with it. The user-script role keeps its options and context meaning. Its replacement emits the same glue, loader and an install call with the sandbox identifier for the extension world. That world's policy must admit `'wasm-unsafe-eval'`, and it is not qualified by this receipt.

### Decisions still open

The first is decided. Each of the others returns for an explicit decision before the implementation issue is dispatched:

1. Decided: srcdoc frames load the glue from a fixed same-origin path, with CORS headers, beside the embedded bundle. Only the glue is served; the module stays embedded inline.
2. Adding `'wasm-unsafe-eval'` to the frame policy that hosts pin.
3. Keeping the three generated call expressions as the only JavaScript text.
4. Qualifying the extension user-script world, which the pinned job cannot exercise.

## Testing decisions

The existing prepared-document, load/execute, artifact-download and browser-export seams are the acceptance boundary. Native tests retain host provider, routing and result behavior; the pinned Chromium job proves actual globals, realms, ordering, CSP and download effects. Keep each crate's single integration test program. **Confidence: proven testing requirement. Evidence: C01, C07-C14.**

All cases below are required acceptance criteria. Their current-source behavior is **proven**; replacement results are **unknown until executed**. The implementation issue must satisfy every row, not merely add tests with these names.

### Providers and messages

| ID | Acceptance criterion | Evidence |
| --- | --- | --- |
| A01 | No handwritten `.js` remains under `crates/*/src`; includes and adjacent authored head/body/tail/no-op programs are removed. Generated output is reproducible from its inputs, and supplied programs still execute. | C01-C02, C08-C09, C11, C14 |
| A02 | A trace proves data-before-runtime collection, merge/serialization error phases, installation order, repeated entries, overwrites, console insertion only on load/execute and first classic-script access. Native property/key/serialization behavior remains observable. | C02, C08-C09, C12 |
| A03 | Artifact witnesses distinguish online success/failure, key order, live global replacement, empty/missing/inherited entries, lowercase versus other filename suffixes, valid/invalid JSON, non-string writes, transmitted MIME and refused offline mutations. Assert promises rather than synchronous throws. | C03 |
| A04 | Attachment witnesses distinguish repeated ids, live replacement, missing ids, truthy/empty extracted text, invalid base64, binary-string text and byte values 0 through 255. Assert synchronous returns/failures. | C04 |
| A05 | Console witnesses cover four methods, zero/many arguments, circular objects, undefined/null and native formatting, local-before-forward order, raw args, repeated wrapping, pending-send fulfillment/rejection and latest-error precedence. Completion is one-shot even for calls in the same turn and never implicitly flushes callbacks. | C05 |

### Bridge, files and navigation

| ID | Acceptance criterion | Evidence |
| --- | --- | --- |
| A06 | Actual iframe bridge witnesses prove listener-before-post, simultaneous request isolation, wrong tag/id ignored, raw success, failure, post failure, captured 30,000 ms deadline and late reply behavior. User-script witnesses prove native fulfillment/rejection, field precedence and no iframe timer/listener. Mutable callback-array replacement remains visible. | C07 |
| A07 | File witnesses cover Blob, Uint8Array, string and other-value branches; MIME requirements/defaults; Blob-read/serialization failures; online response error; retained byte identity; and actual offline download bytes/name/type with anchor click and URL revocation. | C06 |
| A08 | Browser input witnesses prove capture interception for effective HTML/SVG links, animated/namespaced href, relative/hash resolution, non-HTTP click pass-through, form action behavior, tolerated location refusal and standalone omission. Removal detaches with the original capture value. | C09-C10, C12 |
| A09 | Missing, foreign, unbound and replaced-frame senders cannot dispatch; current-frame ready/error settles once. Preserve router response precedence, provider/consumer ordering and existing bootstrap-error delivery assertions. The child type/id policy is tested separately, not claimed as authentication. | C07, C10 |

### Documents, delivery and lifecycle

| ID | Acceptance criterion | Evidence |
| --- | --- | --- |
| A10 | Browser execution proves await/return/import/throw, `import.meta.url`, relative imports, callback invocation order/booleans, synchronous-throw stopping, Promise.all rejection behavior and preserved user outcome. Completion never precedes the whole authored HTML source. | C05, C08-C09, C11-C12 |
| A11 | Preserve head/html/fragment selection, native recovery, font reset, quoted keys, closing-script/CR/NUL/Unicode/marker-looking input results, base URL and parser/module/defer/DOMContentLoaded observations. Replace carrier/boot text assertions only with witnesses for the behavior they protected. | C08-C09, C12 |
| A12 | A saved policy matrix records both loading paths, inherited/URL policy, exact proposed directive/resource changes, WebAssembly permissions, hash admission/refusal and Trusted Types outcomes. No forbidden policy relaxation or source-authority change is present. Any necessary difference has an explicit reviewed decision. | C01, C10, C12-C13 |
| A13 | With networking disabled, a real downloaded single HTML file runs selected helpers and supplied content, reads snapshot artifacts/attachments and returns a file without parent traffic or sibling resources. Matching glue/wasm is packaged through the normal author path. Record optimized/raw/compressed sizes and recheck actual-size startup; no numeric performance threshold is invented. | C03-C04, C06, C08, C11, C13 |
| A14 | Existing cancellation, dropped-future, start/end-hook, validation, one-terminal-result, raw-value/file-identity and retryable-cleanup witnesses pass. The captured production execution-deadline callback retains its 120,000 ms policy. Startup failure/removal does not leak listeners, timers, frame state or a hidden parent. | C07-C10, C12 |

Write changed tests first and retain failing/passing logs. A parity witness that already passes the old implementation needs a failed-and-restored mutation demonstrating the regression it catches. Examples include skipping an installer, reversing provider order, early completion, disabling the source gate, corrupting a byte or omitting a required policy permission. The source-policy guard must fail on the ten old assets before deletion. A compile failure or unchanged passing test is not behavioral red-first proof. **Proven requirement: C01, C14.**

Replace complete source equality, boot/carrier text and generator-output snapshots only where they test implementation. Preserve their identifier/Unicode/input corpus and the security/order assertions. Keep test control Rust-authored; supplied programs remain content, and generated bindings remain generated. Do not broaden this issue into a second browser-runner rewrite. Tests wait for completion signals or captured production callbacks, not sleeps, timed receives or a new test deadline. Keep changed cases under 30 seconds. **Proven requirement: C01, C12-C14.**

Run the repository check, affected native crate tests/doctests, actual wasm and example builds, reverse dependents, tooling/conventions suites when touched, and the pinned browser qualification. Save full command output and exit codes. Retain the 80% line-coverage merge gate; compilation or browser success alone does not establish coverage. Both independent reviews and shared CI remain required. **Proven requirement: C14.**

## Out of scope

1. Rewriting supplied JavaScript, removing user-script messaging or reducing any selected provider capability. **Proven scope: C01-C07.**
2. A new extension host, transport, scheduler, store, worker isolation model, hard-interruption promise or runtime consumer build. **Proven boundaries: C07-C11, C13.**
3. Extra libraries, dependency features, compatibility formats, root README redesign or unrelated browser qualification. Existing broader qualification remains tracked by #825 and #1084. **Proven scope: C01, C13-C14 and the sandbox guide.**

No useful capability is omitted. The deleted assets and script envelopes are implementation mechanisms replaced by the qualified Rust path. Keep corrected empty-artifact lookup, native SVG navigation and whole-source completion ordering; do not restore the defects their witnesses prevent. **Proven: C03, C09, C11-C12.**

## Further notes

### Documentation and glossary

**Confidence: proven requirement. Evidence: C01, C13-C15.**

The first spec commit replaces the R11 sentence permitting an embedded authored JavaScript runtime with the Rust-built, generated-binding contract, without changing the frame's authority or cooperative interruption limits. Link this page as its detailed sandbox contract.

Update the sandbox feature page in the same implementation PR. Open with a quick start for the delivered API, then cover providers, globals/messages, online/standalone modes, prebuilt assets, CSP/Trusted Types, sender policy, completion and cleanup, followed by examples. Remove obsolete boot hashes/carrier/source-generation instructions only after their security and literal-text behavior is covered. Update browser-tools, artifact download, browser-build, development, architecture and the browser-library record wherever their actual paragraphs change. Document only the qualified policy and compiled API.

Give every changed item line rustdoc, including private installers, errors, ownership and public usage examples. Review repository AGENTS.md for stale how-to instructions; no frozen rule change or gratuitous edit is requested. Record a no-change result when its instructions remain correct. Humanize every edited document in Maestro's voice before commit.

Update the existing **Sandbox runtime provider** glossary definition to: "A retained source of owned globals, an ordered runtime selection, helper descriptions and optional lifecycle/message hooks. Built-ins select Rust implementations; supplied content remains executable input."

Add **Sandbox runtime bundle**: "The matching prebuilt WebAssembly runtime and generated browser bindings used to install selected sandbox helpers in an isolated document, including standalone output." Keep the existing Sandbox host, Sandbox value and Browser SandboxIframe names; add no synonym.

### Open questions and required receipts

Each entry names its owner, its blocking observation and what the [bootstrap qualification receipt](#bootstrap-qualification-receipt) settled.

| Question | Owner and closure |
| --- | --- |
| Which generated bootstrap preserves first-script ordering and the current CSP/Trusted Types observations in both loading paths? | Bootstrap spike. Save the measured matrix, proposed policy text and approved mechanism before implementation dispatch. C08-C09, C12. **Measured:** parser-blocking generated startup on both paths; srcdoc frames load served glue, as decided. |
| How are asynchronous user bodies and supplied runtime functions executed while preserving return/await, import.meta, relative imports and current policy refusal? | Bootstrap spike. Execute actual programs in the isolated realm; generic Function/eval permission is not assumed. C02, C08, C12. **Measured:** a module envelope settled in Rust; supplied providers run from inert carriers. Keeping the envelope is an open decision. |
| What replaces public bridge source generation, and how does the immutable matching bundle reach each constructor/default and standalone download without a build cycle? | Bootstrap spike. Publish exact signatures, native/browser availability, all caller edits and a normal-build/offline witness. C02, C07-C08, C11, C13. **Named:** a checked-in generated bundle with unchanged constructors; reproducible prototype builds and an offline witness. The user-script world stays unqualified. |
| Does the proposed runtime fit the qualified bootstrap, and does the actual release bundle pass? | Bootstrap spike measures the largest module that installs before classic scripts under CSP and records a supported full-runtime size estimate beside it; an exceeding estimate blocks dispatch. Implementation reruns identical witnesses against the actual optimized all-provider bundle and returns any failure for a decision. C01, C12-C13. **Measured:** 8 MiB limit against an estimate of at most about 0.93 MB; the actual bundle remains for the implementation issue. |
| Does removing the frame during startup stop installation and settle the host once? | Bootstrap spike. Remove the frame while the selected loader starts. S11, C08-C10, A14. **Not qualified:** no witness removes a frame during startup. |
| Do startup, startup failure and removal leave no native resources behind? | Bootstrap spike. Observe listeners, closures, timers, frame state and hidden parents after the selected loader starts, fails or is removed. C07-C10, A14. **Not qualified:** no witness checks for leaked native resources. |
| Does the current frame's one-shot ready or error handshake settle once? | Bootstrap spike. Send the handshake twice, and from a replaced frame, under the selected loader. C07, C10, A09. **Not qualified:** no witness settles the handshake under the selected loader. |
| Do helpers stay correct when initialization is delayed? | Bootstrap spike. Start the runtime after the parser has passed the loading point and trace first-script access and order. C08-C09, S11. **Not qualified:** no witness delays initialization. |
| Does completion wait for the whole authored source? | Bootstrap spike. Place authored source after `</html>` and order completion after it. C11-C12, A10. **Not qualified:** no witness orders completion after trailing source. |

### Public source evidence

These citations establish requirements or source behavior, not passing replacement tests. Paths reflect the inspected tree; coordinated owner renames update citations rather than retaining duplicate modules. `A` below is the public path `crates/maestro-app/src/browser/components/sandbox`; `W` is `crates/maestro-web/src/components`; `I` is `W/sandboxed_iframe`; `B` is `W/sandbox/runtime_message_bridge`.

| ID | Confidence | Public evidence |
| --- | --- | --- |
| C01 | proven | #3973 and its parent #3768: Rust runtime, retained globals/messages/order/sender checks, supplied content, generated glue, reviewed CSP and docs. |
| C02 | proven | `A/sandbox_runtime_provider.rs:15-57`; built-in `get_runtime` implementations; `crates/maestro-web/src/contributions/adapters.rs:253-269`. |
| C03 | proven | `A/artifacts_runtime_provider/runtime.js:1-82`; host owner `A/artifacts_runtime_provider.rs:141-265`; `crates/maestro-app/tests/artifact_runtime_access.rs`. |
| C04 | proven | `A/attachments_runtime_provider/runtime.js:1-31`; data owner `A/attachments_runtime_provider.rs:21-49`; app `tests/sandbox_runtime_providers.rs`. |
| C05 | proven | `A/console_runtime_provider/runtime.js:1-93`; host owner `A/console_runtime_provider.rs:77-160`; web execution witnesses. |
| C06 | proven | `A/file_download_runtime_provider/runtime.js:1-51`; host owner `A/file_download_runtime_provider.rs:28-78`; `I/browser/values.rs:62-98`. |
| C07 | proven | `W/sandbox/runtime_message_bridge.rs:4-65`; `B/{runtime,sandbox_iframe,user_script}.js`; `W/sandbox/runtime_message_router.rs:89-99,125-154`; its `browser.rs:114-140,194-269` and sender tests. |
| C08 | proven | `I/browser/mod.rs:90-120,179-215`; `I/runtime.rs:9-33`; `I/document.rs:4-59`; `I/tests.rs`. |
| C09 | proven | `I/navigation.js:1-34`; `I/repl-{prefix,suffix}.js`; `I/browser/frame.rs:14-104`; existing browser event-subscription owner. |
| C10 | proven | `I/browser/messages.rs:28-89`; `I/browser/execution.rs:105-319`; `I/execution.rs:34-82`; `I/browser/host.rs:9-51`; `I/browser/values.rs`. |
| C11 | proven | `crates/maestro-web/src/tools/artifacts/html_artifact.rs:131-159`, its `header.rs:144` and `body.rs:208`; `src/contributions/adapters.rs:204-222`; `src/tools/javascript_repl.rs:28`. |
| C12 | proven | Web `tests/sandbox_loading_browser.rs:542` onward; `tests/sandbox_loading_support/programs.{rs,js}`, especially `programs.js:55-95,97-151`; `tests/sandbox_execution_browser.rs`, `tests/native_html_artifact_browser.rs`, `tests/native_remaining_artifacts_browser.rs`; existing fixtures in `tests/fixtures/`. |
| C13 | proven | `crates/maestro-web/Cargo.toml`; `crates/maestro-tooling/src/browser_author.rs`, `src/browser_export_setup.rs`; `justfile` browser recipes; `docs/libraries/browser-frontend.md:107-154`; Rust browser qualification prerequisite #3972. |
| C14 | proven | Repository `AGENTS.md` checks, documentation and test discipline; `docs/specs/maestro-port.md` R11 and graph; existing `maestro-test-conventions` checker. |
| C15 | proven | `CONTEXT.md` Sandbox runtime provider/host/value and Browser SandboxIframe terms; `docs/browser/sandbox.md`; `docs/application/browser-tools.md`; `docs/identity.md` and documentation requirements. |

The runtime source paths above are deletion targets and historical source evidence for this contract. Refresh those citations to the Rust owners when implementation lands; do not retain authored assets to keep links alive.

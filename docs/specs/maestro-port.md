# Maestro: complete Rust foundation

## Purpose and authority

Build the complete model, agent, application, terminal, command-line, RPC and browser foundation in Rust. Deliver the existing behavior through Rust APIs, an owned normal-scrollback terminal interface and an egui/eframe browser interface. A toolkit substitution does not authorize a missing capability. Replace contradictory implementations at their owning modules; do not add compensating caller-side policies.

This specification is the single current foundation contract. Earlier model/engine specifications and conflicting ticket appendices are retired when it lands. Public requirements R01–R12 organize delivery; they do not narrow exported APIs, accepted inputs, documentation, examples, fixture coverage or platform behavior within those areas. The source-derived acceptance matrices attached to each implementation review decide fine-grained semantics. An unresolved library or service destination blocks the affected delivery, not its inclusion in scope.

Approved adaptations are Rust implementation and idioms; the specified UI toolkits; one author-built WebAssembly component extension format with the approved native-async runtime; release-generated changelogs; mise/just/prek developer tooling; syntect highlighting with approved token-color differences and preserved theme keys; and katex-rs browser math with matching CSS/fonts. The common semantic and boundary contract governs observable results and the use of Rust libraries. Other application behavior changes require a recorded owner decision. Existing format conversions, accepted old tool inputs, conventional context-filename aliases, search-helper acquisition, npm/Git installation and self-update remain required behavior.

## Common semantic and boundary contract

1. **Mapped files, idiomatic Rust, preserved observations.** Each named reference file maps to its Rust module, with exported/domain names in Rust casing. Rewrite each file internally using Rust ownership, typed records and enums, pattern matching, `Result`/`?`, iterators, traits, the standard library and the best native crates; do not mirror private helpers or statements. Observable behavior and results match the reference exactly: inputs, defaults, outputs, messages, formats, events, retained-object observations, cancellation points, persistence timing and mode-specific control flow. Choose Rust libraries and standard-library facilities that match; where a library result differs, Rust code produces the reference result without imitating engine internals, foreign engine limits or library internals. Only corrected reference defects, including engine artifacts that are themselves bugs, differ; list each correction once in the owning delivery. Preserve meaningful wire omission/presence rules with typed Rust data. Test application behavior at its owning API.
2. **One owner per behavior.** Small capability ports hide providers, files, processes, clocks, random sources, terminals, browser operations and component engines. Production and controlled adapters obey the same public conformance cases. No frontend imports another frontend, drives a model directly, owns application policy or selects an implementation behind a caller's back. Every frontend calls `maestro-app`. Exact internal dependency sets are CLI and chat: app, tui, tui-crossterm, theme; RPC and web: app, theme. The toolkit and theme are presentation libraries, not sibling frontends. Configuration/session/session-search selectors remain chat-owned; CLI reaches their injected presenter through the app interface wired by the executable. No other internal frontend dependency is allowed. The executable only wires adapters and starts a frontend.
3. **Preserve operational boundaries.** Dropping a result handle is not a universal abort. Cancellation is cooperative where callbacks are cooperative; it does not force all authentication, retry waits or guest work into a new race. Persistent writes are not upgraded into transactions. Do not add rollback, poisoned-session states, admission batches, restrictions, budgets or timeouts where none exist.
4. **Keep current data plus the existing conversions only.** Support the established session, settings, startup and keybinding conversions and accepted edit inputs. Add no general migration/refusal/archive/rollback layer. Model/provider names, resource kinds, extension registrations and vocabularies remain extensible data, not closed code enums used as policy.
5. **Verify before completion.** Each ticket maintains a per-file manifest mapping application behaviors, exports, source test declarations, parameterized cases and fixtures to Rust behaviors/tests, not a one-to-one inventory of private statements or helpers. Many-to-one internal implementation mappings are valid when they cover all required behaviors. Preserve live-only and skipped-test reasons without using them to avoid deterministic tests. Require red-first public-interface tests, strict rustdoc, formatting, Clippy, convention checks, two-axis review and shared CI. Runtime module-resolution probes have an explicit non-applicability row under static Rust linkage; they are inventoried, not claimed as passing runtime tests. Numerical inventory coverage is not evidence that runtime parity passes. Require at least 80% line coverage from tests that each prove real behavior, with names stating that behavior; no fake, tautological, prose-snapshot, duplicate or implementation-detail tests.
6. **No debt.** Every review finding, Minor included, blocks landing and is resolved in the same pull request. Delete slop rather than defer it to follow-up, polish or later-release issues. Existing polish items are resolved in their owning fix or closed as obsolete. Explicitly owner-scheduled platform checks are not debt.
7. **Mechanically enforced quality.** Each function has at most five parameters, including at most one boolean parameter, at most 60 lines, cognitive complexity at most 15 and nesting at most four levels. Each Rust file has at most 500 production lines; a longer reference file becomes a folder module while retaining its file mapping. Keep pedantic lints clean. Production code uses no `unwrap`, `expect` or `panic`; forbid unsafe code. Enforce this bar mechanically through the repository lint configuration and conventions checks.
8. **Source-sized production code.** State the reference lines ported and planned production size before implementation; aim at no more than 1.5 times those lines. Above 1.5 times, map every extra block to a reference line or named Rust necessity and delete unmapped blocks. Larger deliveries are acceptable when every extra block is fully justified in the delivery report.

9. **Port with judgement.** Port everything that contributes to the reference's good behavior. Do not port pieces that do not work or add no value: dead or unreachable code, unused exports, stale or wrong comments and documentation, broken example flags and valueless quirks. List each omission once with its reason.
10. **Design splitting, naming and reuse.** Module, file and test names say what they hold; shared pieces are reused rather than duplicated. Maestro identity appears in names, comments and documentation. Reviews challenge crate choices, naming conventions, file and test names, splitting, reuse and identity.

**Keep reference domain names.** Use Rust casing for AgentSession, ModelRegistry, SessionManager, agent_session.rs and build_session_context. Replace only reference-specific branding with Maestro branding. Test and script file names and test titles must never be identical to the reference’s; describe their behavior in Maestro’s own words. Drop leading issue numbers. Exported/domain naming allows only Rust-forced exceptions, including module-inception and keyword handling. Private helper names, loop forms, allocation strategies and internal decomposition follow idiomatic Rust. Crate and directory names stay unchanged. CONTEXT.md follows the reference domain terms; landed code adopts those names during its re-port. Keep CLI flags, settings keys, RPC commands, event names, session-file fields, tool names and provider/model IDs unchanged. Both Standards and Spec reviewers enforce this rule.

## R01 — Models, providers and authorization

Provide public model/message/content/tool/schema/usage/error records; protocol registration with ordered replacement and source cleanup; raw and simple invocation paths; independently produced event streams and same-invocation result observation; serialization, request hooks, diagnostic records, session resources and all helper operations. Callers can invoke a supplied descriptor without catalog membership. Returned event handles retain their cumulative shared-state observations; pre-invocation failures stay distinct from stream failures. Explicit stream end without a terminal result does not invent one.

Each protocol adapter owns its projection, wire conversion, authentication defaults, setup/retry behavior and usage/cost interpretation. Preserve the two-pass tool-ID projection, including omitted failed assistants and retained tool details. Partial arguments use repair/best-effort parsing during streaming and finalization; execution validation is later and gives corrective field/name/received-value feedback. Direct payload/response callback failures fail the invocation; the response callback runs before the first event/body processing, not for every intermediate setup retry. Header layers use ordered case-insensitive replacement, including repeated names in one layer.

Ship every catalog provider and all nine bundled real protocol adapters: message, chat-completion, conversation, response, cloud response, response-session, content-generation, cloud content-generation and cloud conversation streams. Wire identifiers and accepted provider/model literals remain unchanged. Include the opt-in scripted simulator with builders, chunking, pacing, usage/cache simulation, request-observing factories and pre-aborted queue consumption. Ship every bundled OAuth/device/refresh flow, registry operation and dedicated model-account command. Generic registration alone does not satisfy bundled-provider delivery.

Keep the complete offline catalog and developer generator. Do not add runtime catalog refresh. Preserve raw versus simple budgets/options and provider-specific payload defaults, omission rules, retry parsing, proxy/environment behavior, stream end-marker/EOF handling, overflow classification and accounting. A clean chat response can retain its initial stop outcome without a finish-reason field; other protocols retain their own completion rules. Successful measured usage can satisfy the public overflow predicate. Test all protocols with synthetic wire fixtures; paid/live calls require separate authorization.

## R02 — Agent ownership and execution

Provide inert construction, explicit replaceable instructions/model/history/tools/callbacks, reset, empty/batched input, active signal, pending tools, last error, streaming state, steering and follow-up modes. Keep independently owned running work when a prompt handle is dropped. Default transport is auto. Low-level Agent listeners are awaited in live subscription order with the active signal; their promises are part of run settlement and waitForIdle. Add/remove during dispatch can affect that dispatch. The application session has a separate synchronous, fire-and-forget public-listener interface; do not conflate the two layers.

Provide raw loop and continuation entry points, the stateful owner and the proxy stream client. Preserve before/after tool boundaries, source-ordered results, parallel/sequential execution choices, preparation before validation, intercepted errors, progress, termination and remaining-tool handling. Ordinary fallible callback errors follow the synthetic error-assistant/history/end-event path before runtime flags clear. Do not turn every Rust panic into a promised recoverable callback error.

## R03 — Credentials, settings and local catalog

Credentials maintain a tolerant accepted snapshot until explicit reload, ordered provider metadata and drainable errors. Authentication availability is distinct from stored/login presentation. Mutations update accepted memory even if persistence fails. Preserve current stored token forms, branch-specific expiry/fallback and refresh locking. Thrown refresh reloads and blocks ambient fallback unless a newly valid stored token exists; a nonthrowing empty refresh can continue normal fallback. Cached secret commands use the whole command string and inherited cwd; per-request configured keys/headers execute fresh and can throw. Listing metadata executes neither.

Settings retain user/project scopes, runtime overrides, established names/defaults, a top-level merge with one nested object spread, immediate cache publication, queued writes, flush and error drain. Reload accepts each healthy scope independently, retains a failed scope and rebuilds effective values without preserving runtime overrides. Use Rust `std::fs::File` locking as-is for native storage mutual exclusion. Keep application read/callback/write ordering, explicit application retry loops, permissions, memory publication and failure handling. Do not reproduce lock-directory leases, heartbeats, mtime probing or library retry internals.

Local model files use partial nested overrides, validated supplied limits and zero defaults for omitted price fields. Dynamic provider replacement/removal is separate and restores underlying catalog entries. Selection preserves distinct command-line, exact-reference and scope matching: saved model within scope first, otherwise first scoped model; catalog-order exact matches where specified; uniqueness only for the exact-reference operation; provider-prefix/raw-ID/suffix/alias/date/glob ordering and warnings. No mandatory manifest, origins or locked values are active in this phase.

## R04 — Session trees, persistence and exports

Native sessions use the established JSONL files, project-path encoding, identifiers, headers, versions and append-only tree entries. Also support explicit memory sessions. Keep one in-memory owner for entries, branch position, labels and resolved context. Append changes memory before persistence; new sessions delay initial file publication until the established assistant condition. Do not add transactional batch admission, uncertain-write disablement or close protocols. Preserve malformed/truncated-file handling, existing conversions, discovery, modification/activity metadata, continue/open/fork/clone/import and progress callbacks.

Support branch traversal, names/labels, custom entries/messages, tool pairing, recorded model/thinking changes, compaction context, abandoned-branch summaries, usage/statistics and selected-branch JSONL export. Compaction preserves cut-point/token-estimate/image/tool-pair rules, exact prompts, retained-budget behavior and event/queue ordering. Import is current-session interchange, not a general migration framework.

Deliver the complete self-contained offline session viewer: escaped data, tree navigation, filtering/search, deep links, collapsed details, syntax/Markdown/image/custom-tool rendering, statistics and JSONL download. Implement its behavior in Rust, with an egui web viewer and embedded assets rather than relying on a network service or dropping browser controls. The document serializer accepts data, render capabilities and a prebuilt viewer asset bundle; it does not import a frontend. The viewer itself belongs to the web frontend and calls the application's read-only session-view interface. Build packaging injects its bundle into the serializer, avoiding an application-to-frontend dependency. User-triggered online sharing is a separate operation with an owner-chosen destination.

## R05 — Tools and process operations

Provide the public read, write, multi-edit, shell, search and directory operations, their definitions, preview/render callbacks, wrappers and injectable operation interfaces. Preserve original-snapshot edit matching and mutation queues. Prepare accepted string-valued edit arrays and top-level replacement pairs before schema validation; preparation does not execute the assistant call. Preserve exact diff, BOM/newline/fuzzy matching, path, byte/line/width truncation, notices and full-output artifact behavior.

Read fits recognized JPEG/PNG/GIF/WebP images only under its setting; a failed fit has the specified text omission. Do not globally resize extension-result images. Block-images filtering belongs in model conversion; clipboard BMP conversion is separate. Search discovers/acquires rg and fd and invokes their established arguments. Preserve application traversal choices, hidden-file handling, stat-failure skipping and result ordering through native ignore/glob, path and locale facilities under the common contract; correct documented search behavior where defective.

Keep direct shell, explicit exec and agent-invoked shell separate. Preserve combined output, update timing, missing exit status, timeout/abort/error output differences, spawn hooks and process cleanup. Keep optional timeouts, seconds-to-duration calculation, nonpositive-disable branches, cancellation precedence and cleanup. Use native async timers without reproducing foreign timer internals; correct erroneous delay clamping and warning artifacts. No universal partial-output-on-failure or termination promise is added. Tool-call interceptor failures block; user-shell hook failures are diagnosed and ordinary dispatch continues. Extension result replacement does not acquire the low-level after-call termination field.

## R06 — Instructions, resources and packages

Provide skill/frontmatter parsing, warnings, name collisions, prompt templates, context discovery, source/provenance data, watch/reload behavior, resource loaders and resource-exported documentation. Ordering is entry-point-specific: application package discovery, configured locals, automatic resources and extra paths are not collapsed into one generic priority rule. Explicit extension sources precede discovered ones. Canonical deduplication/name collisions retain the first entry in the relevant order. Search conventional context input names in order: `AGENTS.md`, `AGENTS.MD`, `CLAUDE.md`, `CLAUDE.MD`; use the first readable file in each directory and preserve diagnostics. These are accepted filename literals, not product branding.

Template expansion parses quoted arguments without shell execution and recognizes positional, slice, all-arguments-name and all-arguments-shorthand placeholders in the original template. Insert argument text literally without rescanning inserted text, correcting the interpolation defect. Preserve positional/slice/missing-argument rules, including zero slice-start handling. There is no default-value syntax or replacement-string metacharacter expansion. Keep first-only replacement where another operation explicitly requests it. A custom system prompt replaces the default documentation block; extension transforms can replace the result. Preserve tools/skills/date/cwd assembly without forcing a documentation pointer back in.

Package installation uses configured npm and Git commands, ordinary dependency/lifecycle scripts and local sources in place. Personal/project/temporary paths and manifest resource filtering remain observable. List both scopes. Treat every explicit version/tag/range/ref as pinned; update skips pinned/local sources before repair. Installation/resolution can restore missing pinned contents; force is not a package-pin bypass. Preserve grouped registry progress, engine-only force, supported npm self-update, and bare/combined self/extensions selections. No consumer-side Rust/Cargo builds occur: shipped executables and components are prebuilt. Browser/framework build tooling and extension author tooling are development operations, not consumer installation.

## R07 — One component extension system

The protocol requires: revise the WIT/SDK for recoverable errors, owned persistent handles, missing members, provider hooks and live themes; demonstrate real import-triggered synchronous factories/event-bus prefixes without polling; demonstrate command-initiated replacement/reload with fresh contexts, disposal, post-return cancellation and release; replace completeness claims with assertion-level evidence for every event rule. Keep the intended event semantics explicit while full algorithms remain implementation work.

Extension events: an event emitted from inside a synchronous callback (an event-bus listener, or a footer, widget, header, editor or component builder) reaches listeners in other extensions right after that callback returns. Every other ordering follows the reference behavior.

Use three jobs: maestro-extensions governs semantics; maestro-extensions-wasmtime executes components through those ports; maestro-extensions-wasm provides guest authoring and the canonical WIT. The guest's only allowed internal dependency is maestro-request, the shared owner of model-request records. The runtime adapter depends only toward the domain and is wired by the executable. The host reads shared WIT build input without linking the guest crate. Real host capabilities, event/discovery conformance, UI lifetimes, SDK/examples/docs and platform gates remain acceptance, not claims proven by ABI probes.

Event payloads and results cross the existing generic native-async export as plain JSON beside their typed capability envelope. Use the shared records' existing serde_json representation, not duplicate guest models or a second serializer format. Finite floating-point numbers roundtrip exactly, including signed zero. Ordinary host serialization turns nonfinite numbers into null; this does not promise that null decodes into a required number. An extension writing Infinity or NaN receives a clear error before its edited event or result is encoded. This explicit error is the accepted difference from retaining the nonfinite value until serialization. No bit-encoded numbers, numeric transport wrappers or compatibility decoder are permitted. Shared records retain their existing optional-field semantics without added Presence wrappers; unshared event fields still preserve their meaningful missing/null distinctions. Preserve separate event-edit and callback-result outcomes when either fails.

Use Wasmtime/wasmtime-wasi 49.0.2 and the Rust SDK bindings for one WebAssembly component system. The application extension world uses WASI 0.3 native async, with WASI 0.2 interfaces needed by Rust standard-library basics. Support an authoring language when its toolchain emits the required component contract. Rust is the initial qualified authoring path; JavaScript/TypeScript authoring is unavailable until its tooling supports that contract. Do not add a polling workaround, a separate legacy source loader, a second host or an out-of-process extension protocol. Runtime JIT is allowed; install/use never invokes Rust/Cargo compilation.

The component SDK preserves every host capability: ordered live events/hooks, transforming and cancelling operations, tool schemas/validation/progress/results, commands and command-only context methods, flags, shortcuts, providers/OAuth/streams, source discovery, session queries/entries, messaging, exec/network/files, live getters, themes, renderers, custom editor/components, focusable overlays, status/footer/header/widgets and reload. UI render/input/invalidate calls remain synchronous where their callers require it; asynchronous work uses native async and owned resources, not JSON-only replies. Use shape A capability interfaces and typed resources; register handlers one by one so the host retains per-handler semantics. Messages, entries, provider payloads and tool arguments use session JSON losslessly. The domain exposes ExtensionRuntime, ExtensionInstance and ExtensionHostApi ports; maestro-app implements the host port. Only the Wasmtime adapter owns its engine/linker/bindings, one actor per generation on a multi-thread Tokio runtime, stream pipes and trap diagnostics. The guest owns one canonical WIT source checked against host code generation. Network operations use the WIT http interface backed by host reqwest, not a second WASI HTTP library. The shared interface hides runtime-library types.

Extensions receive the user's full effective permissions through host operations. Add no capability grants, restriction policy, fuel, memory limits or deadlines. Preserve registration-specific collision behavior: same-extension replacement, cross-extension rules and individually diagnosed provider failures; there is no whole-owner atomic validation policy. Hooks iterate current collections in load/registration order. Presence of UI does not imply every mode implements the same methods, and every disposal/reload does not automatically settle dialogs.

A replacement generation becomes current without exposing mixed-generation registrations, while admitted old command frames keep their old resources until they finish. This is internal ownership, not an observable admission-policy change. Reconstruct new guest state through the same session state/hooks, not heap migration. Validate stale/foreign/disposed handles before Wasm entry; preserve operation-specific in-flight cancellation and reentrancy. Initially retain compiled components only in a process-local identity-bound cache; never deserialize native bytes from packages, users or disk. Compile the original component on a miss. A persistent native cache needs a separate integrity proof: full-user extensions can modify same-user files, so filesystem ownership alone is not a trust boundary. Stale or not-yet-bound context calls return recoverable, catchable errors, not traps; one caught stale call must not terminate another task. Traps are reserved for actual component faults and do not justify calling a poisoned instance again.

Release qualification must exercise the complete Rust native-async SDK with actual extensions, pending provider streams and host I/O; full transform/veto/cancellation/progress semantics; reload during old commands; failed activation and partial provider errors; widget focus/theme/Unicode/invalidation/disposal; distinct-component memory and repeated reload cleanup; and platform/license checks. Preliminary registration/clock probes do not prove the complete SDK.

## R08 — Application, SDK and lifecycle

`maestro-app` is the one application interface and the embedding SDK. It composes model/agent/session/credential/catalog/resource/tool/extension/package services through ports. It owns prompt delivery, steering/follow-up publication, events, model/thinking selection, compaction, visible retries, direct-shell transcript timing, navigation/import/export, source reload and session replacement. Frontends select presentation modes, not alternate business rules. Browser-specific tool/artifact/storage policy also lives here, using browser adapters supplied by the web frontend.

Preserve lifecycle-specific veto/preparation/shutdown/disposal/rebinding order. There is no universal abort-and-idle gate or rollback after teardown. Interactive reload refuses streaming/compaction; direct SDK reload preserves its distinct lifetime. Application retries classify nonempty assistant error text after excluding overflow; do not add unconditional authentication/billing/quota exclusions. Aborted outcomes do not retry. Keep application retries separate from transport setup retries. Preserve queue recovery, active context replacement, event bus semantics and costs added at the defined message boundary.

The browser build uses the same application crate with capability-selected native/web adapters. Native process, file and component-engine implementations are not linked into a browser just because the application exports their contracts. The browser surface does not acquire native operations it never exposed. This target separation requires an actual wasm build/smoke test, not a conditional-compilation claim alone.

## R09 — Terminal toolkit and interactive frontend

Provide a reusable terminal library with no UI framework and internal dependencies limited to shared cancellation and the optional path utility, as defined in the [crate graph](#crates-and-delivery-order). Port the owned retained-line frame writer and all twelve components: box, cancellable loader, editor, image, input, loader, markdown, select list, settings list, spacer, text and truncated text. Preserve normal scrollback, differential redraw/resize/clear/purge, synchronized updates, raw OSC8/image escapes and hardware cursor/IME location. Use the terminal adapter crate through the terminal port, which on Unix drives the standard descriptors directly; inject a clock for the 16 ms render throttle. Preserve async raw input, Kitty keyboard, bracketed paste and COLUMNS/LINES fallback. The standalone virtual-terminal harness depends only on the toolkit; no crate depends on that harness. Require the complete source scenarios plus a real Linux tmux cross-check.

Preserve synchronous component render/input/invalidate, retained component state, focus routing, live input listeners, overlay positioning/z-order/capture/disposal, background/style boundaries and custom editors. Port raw/Kitty/legacy key decoding, release filtering, fragmented escapes and bracketed paste. The editor preserves actions, grapheme movement, sticky visual-column movement, history, atomic paste markers, kill/yank, undo, completion cancellation, fuzzy ranking and file/slash completions using native segmentation and width libraries with Rust-valid string boundaries under the common contract. Do not introduce a UTF-16 string/event substrate. Port every widget, Markdown block/inline behavior, image protocol, theme/palette, watcher and image fallback. Highlighting belongs to the shared theme library and uses syntect 5.3.0 with two-face 0.5.2+bat-0.26.1, defaults disabled and syntect-fancy enabled, never the onig C library. Keep the theme format and its nine syntax color keys, mapped from scopes. Every terminal/browser/viewer consumer uses that interface, with an injected callback keeping the terminal toolkit independent of highlighting libraries. In the terminal, resolve only explicit language names/extensions and data-defined aliases in the selected syntax set; disambiguate collisions and never auto-detect. Absent/fancy-excluded languages using the terminal plain fallback are recorded accepted differences. Missing or unsupported terminal languages return mdCodeBlock-colored lines. Preserve the distinct direct-helper raw-line error fallback and the Markdown callback's mdCodeBlock-colored error fallback. Token colors may differ; no old-engine token-parity requirement remains.

Code highlighting follows each caller: the terminal renders unlabeled or unknown-language code blocks as plain text; the exported session viewer and the browser code blocks select the language automatically when it is missing or unknown.

The interactive frontend renders the full transcript, custom/tool rows, thinking toggles, selectors/tree/session search, editor, footer/status/loaders and all extension UI operations. Deliver every retained command, clipboard text/images, account dialogs, suspension/recovery, retained visual components, resource notices, update guidance and diagnostics. SIGTERM graceful shutdown exits zero; emergency hangup/dead-terminal cleanup exits 129. Mode-specific input admission, replacement and flush semantics remain separate. Application-owned theme, editor and toolkit gaps require fixes and regression traces; library results obey the common contract without reproduction adapters.

## R10 — Rust CLI, print/JSON and RPC

Preserve prompt-first argument grammar, aliases/flags, stdin/file/image initial input, list/account/package commands, environment/bootstrap behavior, startup conversions, mode selection and exit rules. Text mode emits the final answer; JSON mode emits the complete established event stream with clean stdout. Print flushing does not imply identical RPC shutdown behavior. The binary contains only wiring.

RPC is JSONL with correlated responses, stream events and these 29 commands. Identifiers are wire literals:

| Area | Commands |
|---|---|
| Execution and state | `prompt`, `steer`, `follow_up`, `abort`, `new_session`, `get_state` |
| Model and queues | `set_model`, `cycle_model`, `get_available_models`, `set_thinking_level`, `cycle_thinking_level`, `set_steering_mode`, `set_follow_up_mode` |
| Recovery and shell | `compact`, `set_auto_compaction`, `set_auto_retry`, `abort_retry`, `bash`, `abort_bash` |
| Sessions | `get_session_stats`, `export_html`, `switch_session`, `fork`, `clone`, `get_fork_messages`, `get_last_assistant_text`, `set_session_name`, `get_messages` |
| Discovery | `get_commands` |

Preserve omitted/null field distinctions, empty last-assistant data `{}`, parse failures without IDs and the actual invalid-value/failure boundary; promise neither arbitrary-JSON survival nor universal shutdown drain. RPC UI has exactly the nine established wire methods: `select`, `confirm`, `input`, `editor`, `notify`, `setStatus`, `setWidget`, `setTitle`, `set_editor_text`; unsupported component operations preserve their documented no-op/absent behavior. Dialog response, supplied cancellation and supported caller timeout retain their individual ownership. The stream has exactly 17 session event types plus extension_error: 18 total. Preserve every source flag, --theme/--no-themes and all 13 short aliases. The newer max thinking level is not in this baseline. Also deliver the typed subprocess client with process/stderr/correlation ownership and its own deadlines; the server alone is not client delivery.

## R11 — Browser frontend, documents and sandbox tools

Deliver an embeddable Rust egui/eframe web application, not a native-only GUI. Preserve chat/editor/attachments, responsive split/overlay layout, streaming updates, scroll behavior, abort/steer/follow-up actions, model/thinking controls, custom message/tool renderers, selection/copy/download/links, localized text and embedding lifecycle/callbacks. Maintain browser-visible text/focus/accessibility and DOM integration where canvas widgets alone cannot reproduce them.

Provide the browser's named stores, metadata/indexes, settings/provider keys/custom providers, session listing/deletion, quota/persistence request behavior and artifact reconstruction through IndexedDB. Preserve all supported local-server discovery, authentication/proxy URL handling, key dialogs/tests and provider settings. Browser server discovery is not the excluded newer core catalog-refresh feature.

Accept the established URL/File/Blob/buffer inputs and text/image/PDF/DOCX/PPTX/XLSX/XLS formats. Preserve original bytes, MIME/name/size, extraction wrappers/order/whitespace, previews, cancellation and errors. PDF attachments use hayro 0.8.0 with hayro-interpret for text and first-page preview; PDF artifacts render all pages at the defined scale with cleanup. Keep application wrappers, fit-160px preview and 1.5 artifact/overlay scales, errors and cleanup exact. PDF text content and layout match the reference on the reference inputs; text segmentation, Type3 text, Arabic order and font rendering are part of that qualification. Do not use pdf-extract. DOCX preview preserves pagination, fonts, headers, footers and notes. Spreadsheet preview preserves sheets, formatted cells, styles and tab switching. HTML/SVG/image/Markdown/text/code artifacts preserve preview/source/download and safe rendering behavior. Browser Markdown also preserves inline and display math for dollar and backslash delimiters, HTML math layout/fonts, parse-error spans and fallback text. Use katex-rs 0.3.0 beside the Markdown consumer with output="html", throwOnError=false and delimiter-specific displayMode. Ship matching KaTeX 0.18.5 CSS/fonts as hashed data through the DOM. Reuse the reference math inputs for named checks of complete expressions, inline errors and browser layout. Formulas match the reference's glyphs, layout and tooltip content. Internal markup, class names, element structure and byte or pixel identity may differ only where not user-visible. A visible difference is a defect to fix in the owning Rust implementation, not an accepted result. Fix defects in the application wrapper before landing; do not add MathML, auto-render extensions or math behavior to terminal/export paths that never invoke it. Libraries must qualify these contracts; extraction-only substitutes do not satisfy document preview.

Recreate DOM iframe sandboxing, injected runtime providers, postMessage bridges/routing, initialization/ready/run/cleanup lifetimes, console capture, attachments, downloads and artifact access from Rust. The browser tool intentionally executes user-supplied JavaScript in its browser sandbox; that is application behavior, not a second extension host. Keep the document-extraction tool, the JavaScript REPL tool (`javascript_repl`, with `title` and `code` parameters), and artifact commands/prompts. The calculation-result renderer remains rendering only, not an executable calculation tool. Do not use the stronger-looking native extension permission model to alter browser sandbox observations.

Provide the browser embedding example, Rust build/asset pipeline, supplied transcript fixtures, developer prompt measurement, localization and browser smoke/interaction tests. There were no automated browser test-suffix files in the inventory; therefore new source-derived browser tests are required, not optional.

### Browser drawing and DOM ownership

Use egui for shell layout/drawing, the 800 px split/overlay breakpoint, IME editor, attachment chips/notices/drop highlight, tool-card controls and dialogs. Rust web-sys adapters behind SandboxHost, OverlayPlacer, AttachmentIntake and Transcript ports own real DOM objects. The web Transcript is a DOM text overlay, preserving native find-in-page, screen-reader text, context menus, touch selection, links/copy and the exact >50/<10 px auto-follow guards. Use the selected native Markdown HTML library, syntect HTML and katex-rs HTML under the common contract. Keep one message model and preserve source selection/stream replacement behavior.

Overlay placement follows the egui rectangle each frame with exact fractional-DPR alignment. Hide or move DOM elements whenever an egui overlay needs their area; restore retained state afterward. Attachment intake supplies focused image paste, a hidden file picker and drop forwarding for DOM-covered editor regions. Retain source validation and the IME Enter-while-composing/Process-key guard.

The sandbox's already-injected in-iframe runtime remains a build-time embedded JavaScript asset. Its Rust host preserves srcdoc flags, router, console, timeout/abort and source cleanup, with sender validation and per-sandbox external-link listener cleanup corrected at the owning adapter. This is not a JavaScript UI or an extension host; a busy loop may block the parent process and timers, so no hard-interruption guarantee is added.

Browser smoke covers Firefox/Safari, real touch/IME and real HiDPI devicePixelContentBoxSize behavior, not only an emulated fallback. Cull long egui lists without removing searchable DOM transcript text. The author build trims fonts and runs pinned wasm-opt, recording bundle/glyph/behavior evidence without a new numeric budget; consumers never build these assets.

## R12 — Documentation, examples, repository tooling and release

Development commands (offline test launcher, source launchers, commit hook, asset copying, browser smoke check) live in `maestro-tooling`, which is never shipped, has wiring-only binaries and exposes its library to integration tests.

Maestro does not auto-close issues or pull requests from new contributors; there is no contributor gate.

Deliver current offline API/usage/model/session/settings/tools/extensions/package/terminal/browser/platform/RPC documentation, every executable SDK/extension/demo example and all fixtures/assets. Examples remain examples: an optional subagent, permission guard, sandbox or planner does not make that capability built-in. Port example functionality into Rust component guests and qualify author builds; do not load legacy source examples. Executable code controls behavior and wire acceptance; correct stale documentation and examples to the delivered behavior, listing each correction once. Do not invent historical Maestro releases.

Give every contribution guide, issue template, automation workflow, prompt, ignore file, package manifest, lockfile, compiler/lint/test/build configuration, profiling/analysis tool and packaging script a named Rust-setup counterpart. Use mise to pin just and prek, just recipes as the commands, and prek for the mandatory commit hook: these are the Rust counterparts of npm scripts and the husky hook. Remove unused jaq from mise.toml and mise.lock in the foundation tooling change. Keep YAML issue templates, workflows and prek configuration, including its YAML, TOML and merge-conflict checks. Keep shared CI, not a second build/quality system. Respect dependency-free leaf crates, declared normal/optional/target/build edges, isolated test helpers, frontend independence and wiring-only executable roots through metadata-based convention tests. Shared workflow changes require a separate issue/clone in their repository; the application ticket cannot silently edit that repository.

Default tests run on Linux with deterministic fake providers/clock/terminal/browser/storage adapters and controlled process adapters; required search integration tests also run real rg/fd on temporary fixtures. Restore macOS and Windows qualification before the first release. Every public item has rustdoc; CI/hook documentation builds deny warnings and missing docs. Run `just check`, `just test` and shared `just ci`; do not introduce mutation testing. Hooks format/re-stage only staged files before `just check`, not the full suite. Pin Rust via rustup and existing `rust-toolchain.toml`; pin developer tools in mise. Review all direct and transitive distribution licenses. Release notes come from conventional commits; implementation PRs do not edit CHANGELOG.


CI must be equal or stronger step by step. Install rg and fd and require real-binary search tests, not just fakes. As #109 specifies, `just test` remains unwrapped; the separate ported non-LLM test script removes the specified provider credentials and sets the agent's auth file aside, restoring it on exit. Keep strict Clippy, public rustdoc, conventions, full action-SHA pins and merge-queue validation. The full suite includes terminal tests.

Define the caller-driven wasm32-unknown-unknown build gate in foundation tooling and activate it for the model library at its first re-port; every later browser-used crate opts in when created. The first real web application slice builds its actual wasm application and a working embedding example immediately. Full browser application/example builds remain required thereafter; cargo check, native-only compilation and empty examples do not qualify. Release builds cover darwin arm64/x64, linux x64/arm64 and windows x64, with matching notes/assets.

Shared-workflow changes require one separate issue and clone in maestro-rust-workflows. As #109 specifies, `just test` remains unwrapped; the separate ported non-LLM test script removes the specified provider credentials and sets the agent's auth file aside, restoring it on exit. Its initial real-tool and wasm-job release plus caller/ruleset re-pin precede foundation/models acceptance; the caller later supplies its real browser build and its own tag-triggered release workflow; the shared companion supplies only generic configurable checks. Keep the application lane out of that repository. Every mapped CI step needs observed evidence before final qualification; a plan or skipped job is not proof of delivered parity.

## Crates and delivery order

The table declares the currently enforced crate graph. The authoritative
shared-record amendment below changes its destination graph; extraction updates
this table and its conventions checks together. An em dash means no internal
dependencies apart from the optional foundation utility described below.

| Crate | One job | Allowed internal dependencies | Layer |
|---|---|---|---|
| `maestro-cancellation` | signal cooperative cancellation | — | below 0 |
| `maestro-extensions-wasm` | provide guest authoring | — | 0 |
| `maestro-models` | supply model invocations | `maestro-cancellation` | 0 |
| `maestro-resources` | discover instruction resources | — | 0 |
| `maestro-settings` | own accepted preferences | — | 0 |
| `maestro-storage` | access transcript bytes | — | 0 |
| `maestro-test-conventions` | verify workspace structure | — | 0 |
| `maestro-tooling` | automate repository development | — | 0 |
| `maestro-tui` | render terminal components | `maestro-cancellation` | 0 |
| `maestro-agent` | run an agent | `maestro-models` | 1 |
| `maestro-credentials` | own accepted credentials | `maestro-models` | 1 |
| `maestro-packages` | manage package sources | `maestro-settings`, `maestro-resources` | 1 |
| `maestro-test-terminal` | exercise terminal scenarios | `maestro-tui` | 1 |
| `maestro-theme` | resolve presentation styles | `maestro-tui` | 1 |
| `maestro-tui-crossterm` | connect a real terminal | `maestro-tui` | 1 |
| `maestro-catalog` | resolve the usable model catalog | `maestro-models`, `maestro-credentials` | 2 |
| `maestro-session` | own conversation history | `maestro-models`, `maestro-agent`, `maestro-storage` | 2 |
| `maestro-tools` | execute tool definitions | `maestro-models`, `maestro-agent`, `maestro-tui`, `maestro-theme` | 2 |
| `maestro-export` | serialize session documents | `maestro-session`, `maestro-models`, `maestro-tools`, `maestro-theme`, `maestro-tui` | 3 |
| `maestro-extensions` | govern extension semantics | `maestro-models`, `maestro-agent`, `maestro-session`, `maestro-catalog`, `maestro-tools`, `maestro-theme`, `maestro-tui`, `maestro-resources` | 3 |
| `maestro-app` | coordinate application operations | `maestro-models`, `maestro-agent`, `maestro-credentials`, `maestro-settings`, `maestro-storage`, `maestro-catalog`, `maestro-session`, `maestro-tools`, `maestro-resources`, `maestro-packages`, `maestro-extensions`, `maestro-export`, `maestro-theme`, `maestro-tui` | 4 |
| `maestro-extensions-wasmtime` | execute artifacts | `maestro-extensions` | 4 |
| `maestro-chat` | present interactive conversations | `maestro-app`, `maestro-tui`, `maestro-tui-crossterm`, `maestro-theme` | 5 |
| `maestro-cli` | present command-line operations | `maestro-app`, `maestro-tui`, `maestro-tui-crossterm`, `maestro-theme` | 5 |
| `maestro-rpc` | present the RPC contract | `maestro-app`, `maestro-theme` | 5 |
| `maestro-web` | present browser interactions | `maestro-app`, `maestro-theme` | 5 |
| `maestro` | compose executable entry points | `maestro-app`, `maestro-cli`, `maestro-rpc`, `maestro-chat`, `maestro-web`, `maestro-extensions-wasmtime` | 6 |

`maestro-path` is the foundation utility for lexical path strings. It sits below
layer 0, has no internal dependencies, and exposes platform-independent path
operations. Native crates may depend on it without changing their delivery layer.
This optional utility edge is ignored when evaluating leaf status and exact
direct-dependency sets; every other dependency rule, including cycle and internal
dev-dependency checks, still applies. The guest authoring crate
(`maestro-extensions-wasm`), component runtime adapter
(`maestro-extensions-wasmtime`) and terminal scenario harness
(`maestro-test-terminal`) and cancellation leaf (`maestro-cancellation`) are
excluded from this permission.

The graph contains 28 crates: the 27 table entries plus the foundation utility.
It permits 87 internal production dependency edges: 64 table edges plus 23
optional utility edges. Eight crates are leaves when utility edges are ignored,
including the utility itself. The utility is delivered before its first consumer.

`maestro-cancellation` owns cooperative cancellation below layer 0, with no
internal dependencies. Only models and the terminal toolkit may depend on it.

### Shared-record amendment

`maestro-request` defines model-request records: model descriptors, messages,
content, usage, stream events and diagnostics, plus `SourceInfo` and `Skill` for
instruction inputs. It owns the declarations and their serialization. Existing
models and resources public paths re-export their moved records. Provider
invocation, scheduling, clock-dependent diagnostics, resource discovery and
provenance construction stay with their existing owners; this is not a general
shared-types container.

The crate is core, sits below layer 0 and depends on no workspace crate, including
`maestro-path`. Only `maestro-models`, `maestro-resources` and
`maestro-extensions-wasm` may depend on it. No other graph permission changes:
the guest still cannot depend on models or resources, the runtime adapter still
targets extensions only, and no internal dev edge is added. Existing delivery
layers stay unchanged; the shared owner precedes its three consumers.

The destination graph has 29 crates, 67 table edges plus 23 optional utility
edges (90 production edges), and seven leaves when utility edges are ignored.
The extraction must update `workspace-crates.json`, graph policy, utility
exclusions and architecture tests together. Tests cover ordinary, renamed,
optional, target and build declarations for the three allowed edges and reject
all other edges, including request-to-path and internal dev dependencies.
Move this amendment into the enforced table and surrounding ownership text when
the extraction lands, removing this temporary subsection rather than retaining
two descriptions of the graph.

### Crate order

A crate is reached when every crate it depends on has its first tickets landed. Each crate's tickets are written when that crate is reached, and each ticket links to this specification. Tickets are not planned in advance for all crates. Same-layer crates run in parallel (at most 4 compiling lanes).

1. **Layer 0:** `maestro-extensions-wasm`, `maestro-models`, `maestro-resources`, `maestro-settings`, `maestro-storage`, `maestro-test-conventions`, `maestro-tooling`, `maestro-tui`
2. **Layer 1:** `maestro-agent`, `maestro-credentials`, `maestro-packages`, `maestro-test-terminal`, `maestro-theme`, `maestro-tui-crossterm`
3. **Layer 2:** `maestro-catalog`, `maestro-session`, `maestro-tools`
4. **Layer 3:** `maestro-export`, `maestro-extensions`
5. **Layer 4:** `maestro-app`, `maestro-extensions-wasmtime`
6. **Layer 5:** `maestro-chat`, `maestro-cli`, `maestro-rpc`, `maestro-web`
7. **Layer 6:** `maestro`

## Completion and explicit later scope

Completion requires the entire file/function/test/fixture matrix, all R01–R12 acceptance, full shipped examples/docs, platform prerequisites and selected service destinations to close with no unapproved behavioral omission. A generic abstraction, a passing subset or a complete file map does not substitute for the delivered capability. The library equivalents are supervisor-selected and qualified on the reference inputs under the common semantic and boundary contract. A mismatch requires matching results through the owning Rust implementation, not an engine port or caller workaround. Added task scope still goes through plan approval.

Deferred model responses, remote core catalog refresh, newer transcript/tool-change replay, nested tool calls, cache warming, project trust and the four later RPC commands are not part of this baseline. Classification, generated images, embeddings/reranking, governed manifest/init/settings locks, native out-of-process extensions, alternate default session storage, jobs and added MCP/supervisor capabilities remain later owner phases. Their absence does not justify omitting baseline examples, browser discovery, JSONL persistence or existing conversions.

**Reference defects.** Correct known and encountered defects in their owning mapped files; list each correction once with a regression. Correct stale documentation to the delivered behavior. Do not mark an affected file complete until its fixes pass; an example or adapter without a mapped owner is a coverage gap, not completed delivery.

**Replaceable visual identity.** One design-token data file supplies Maestro colors, font and mark metadata to browser canvas, DOM overlays, the exported viewer and shipped terminal dark/light defaults. Embed/subset Barlow Condensed and JetBrains Mono with OFL notices for browser/viewer text. Use the Maestro M mark. A same-format user override replaces the identity without code changes. Preserve layout, behavior, terminal theme format/names/keys and custom themes; the terminal font remains user-controlled.

**CI placement.** Shared CI is the whole merge check. The caller's `.github/ci.toml` lists `wasm_crates`, four platform `check_targets` and optional `browser_build`; absent configuration skips only optional jobs, invalid configuration fails. As #109 specifies, `just test` remains unwrapped; the separate ported non-LLM test script removes the specified provider credentials and sets the agent's auth file aside, restoring it on exit. Required shared jobs use real rg/fd, wasm builds and foreign-target checks. Enabling the browser application runs the caller's mise-pinned `just browser-build` recipe for real app/example assets. Release binaries, notes, checksums, SBOM/attestation and smoke belong to this repository's tag workflow. macOS/Windows runtime tests return before first release. No separate gate program or extra required repository workflow is added.

**Approved scope exclusions.** Hidden joke commands and their views are not part of Maestro. Omit their dedicated assets and model-selection checks. Rust file paths use the current source-keyed name map; source-based ownership does not change.

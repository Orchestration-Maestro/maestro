# Maestro: complete Rust foundation

## Purpose and authority

Build the complete model, agent, application, terminal, command-line, RPC and browser foundation in Rust. Deliver the existing behavior through Rust APIs, an owned normal-scrollback terminal interface and a Rust browser interface built from native page elements. A toolkit substitution does not authorize a missing capability. Replace contradictory implementations at their owning modules; do not add compensating caller-side policies.

This specification is the single current foundation contract. Earlier model/engine specifications and conflicting ticket appendices are retired when it lands. Public requirements R01–R12 organize delivery; they do not narrow exported APIs, accepted inputs, documentation, examples, fixture coverage or platform behavior within those areas. The source-derived acceptance matrices attached to each implementation review decide fine-grained semantics. An unresolved library or service destination blocks the affected delivery, not its inclusion in scope.

Approved adaptations are Rust implementation and idioms; the specified UI toolkits; one author-built WebAssembly component extension format with the approved native-async runtime; release-generated changelogs; mise/just/prek developer tooling; syntect highlighting with approved token-color differences and preserved theme keys; and katex-rs browser math with matching CSS/fonts. The common semantic and boundary contract governs observable results and the use of Rust libraries. Other application behavior changes require a recorded owner decision. Existing format conversions, accepted old tool inputs, conventional context-filename aliases, search-helper acquisition, npm/Git installation and self-update remain required behavior.

## Common semantic and boundary contract

1. **Mapped files, idiomatic Rust, preserved observations.** Each named reference file maps to its Rust module, with exported/domain names in Rust casing. Rewrite each file internally using Rust ownership, typed records and enums, pattern matching, `Result`/`?`, iterators, traits, the standard library and the best native crates; do not mirror private helpers or statements. Observable behavior and results match the reference exactly: inputs, defaults, outputs, messages, formats, events, retained-object observations, cancellation points, persistence timing and mode-specific control flow. Choose Rust libraries and standard-library facilities that match; where a library result differs, Rust code produces the reference result without imitating engine internals, foreign engine limits or library internals. Only corrected reference defects, including engine artifacts that are themselves bugs, differ; list each correction once in the owning delivery. Preserve meaningful wire omission/presence rules with typed Rust data. Test application behavior at its owning API.
2. **One owner per behavior.** Small capability ports hide providers, files, processes, clocks, random sources, terminals, browser operations and component engines. Production and controlled adapters obey the same public conformance cases. No frontend imports another frontend, drives a model directly, owns application policy or selects an implementation behind a caller's back. Every frontend calls `maestro-app`. Allowed internal dependency sets are CLI and chat: app, tui, tui-crossterm, theme; multiplexer: app, protocol, tui, tui-crossterm, theme; RPC and web: app, theme. Each frontend must depend on app; other allowed edges are declared only when used. The toolkit and theme are presentation libraries, not sibling frontends. Configuration/session/session-search selectors remain chat-owned; pure retained session-tree ordering, geometry, content extraction/search and raw-record ancestry belong to `maestro-session`, re-exported through `maestro-app`; CLI reaches their injected presenter through the app interface wired by the executable. No other internal frontend dependency is allowed. The executable only wires adapters and starts a frontend.
3. **Preserve operational boundaries.** Dropping a result handle is not a universal abort. Cancellation is cooperative where callbacks are cooperative; it does not force all authentication, retry waits or guest work into a new race. Persistent writes are not upgraded into transactions. Do not add rollback, poisoned-session states, admission batches, restrictions, budgets or timeouts where none exist.
4. **Keep current data plus the existing conversions only.** Support the established session, settings, startup and keybinding conversions and accepted edit inputs. Add no general migration/refusal/archive/rollback layer. Model/provider names, resource kinds, extension registrations and vocabularies remain extensible data, not closed code enums used as policy.
5. **Verify before completion.** Each ticket maintains a per-file manifest mapping application behaviors, exports, source test declarations, parameterized cases and fixtures to Rust behaviors/tests, not a one-to-one inventory of private statements or helpers. Many-to-one internal implementation mappings are valid when they cover all required behaviors. Preserve live-only and skipped-test reasons without using them to avoid deterministic tests. Require red-first public-interface tests, strict rustdoc, formatting, Clippy, convention checks, two-axis review and shared CI. Runtime module-resolution probes have an explicit non-applicability row under static Rust linkage; they are inventoried, not claimed as passing runtime tests. Numerical inventory coverage is not evidence that runtime parity passes. Require at least 80% line coverage from tests that each prove real behavior, with names stating that behavior; no fake, tautological, prose-snapshot, duplicate or implementation-detail tests.
6. **No debt.** Every review finding, Minor included, blocks landing and is resolved in the same pull request. Delete slop rather than defer it to follow-up, polish or later-release issues. Existing polish items are resolved in their owning fix or closed as obsolete. Explicitly owner-scheduled platform checks are not debt.
7. **Mechanically enforced quality.** Each function has at most five parameters, including at most one boolean parameter, at most 60 lines, cognitive complexity at most 15 and nesting at most four levels. Each Rust file has at most 500 production lines; a longer reference file becomes a folder module while retaining its file mapping. Keep pedantic lints clean. Production code uses no `unwrap`, `expect` or `panic`; forbid unsafe code. Enforce this bar mechanically through the repository lint configuration and conventions checks.
8. **Source-sized production code.** State the reference lines ported and planned production size before implementation; aim at no more than 1.5 times those lines. Above 1.5 times, map every extra block to a reference line or named Rust necessity and delete unmapped blocks. Larger deliveries are acceptable when every extra block is fully justified in the delivery report.

9. **Port with judgement.** Port everything that contributes to the reference's good behavior. Do not port pieces that do not work or add no value: dead or unreachable code, unused exports, stale or wrong comments and documentation, broken example flags and valueless quirks. List each omission once with its reason.
10. **Design splitting, naming and reuse.** Module, file and test names say what they hold; shared pieces are reused rather than duplicated. Maestro identity appears in names, comments and documentation. Reviews challenge crate choices, naming conventions, file and test names, splitting, reuse and identity.
11. **One thrown-error form.** `DiagnosticErrorInfo` is the Rust form of an error the reference throws, as well as the snapshot a recorded diagnostic keeps. Its name, message, stack and code are the fields reference handlers read from a thrown value, and `extract_diagnostic_error` takes the snapshot. A Rust operation returns it wherever the reference function throws, and so do caller-supplied tools, listeners, hooks, stream functions and extension handlers. A crate defines its own error type only where the reference tells thrown errors apart, by class or by a name, code or message that decides what happens next, as `BashError` and `MissingSessionCwdError` do.

**Keep reference domain names.** Use Rust casing for AgentSession, ModelRegistry, SessionManager, agent_session.rs and build_session_context. Replace only reference-specific branding with Maestro branding. Test and script file names and test titles must never be identical to the reference’s; describe their behavior in Maestro’s own words. Drop leading issue numbers. Exported/domain naming allows only Rust-forced exceptions, including module-inception and keyword handling. Private helper names, loop forms, allocation strategies and internal decomposition follow idiomatic Rust. Crate and directory names stay unchanged. CONTEXT.md follows the reference domain terms; landed code adopts those names during its re-port. Keep CLI flags, settings keys, RPC commands, event names, session-file fields, tool names and provider/model IDs unchanged. Both Standards and Spec reviewers enforce this rule.

## R01 — Models, providers and authorization

Provide public model/message/content/tool/schema/usage/error records; protocol registration with ordered replacement and source cleanup; raw and simple invocation paths; independently produced event streams and same-invocation result observation; serialization, request hooks, diagnostic records, session resources and all helper operations. Callers can invoke a supplied descriptor without catalog membership. Returned event handles retain their cumulative shared-state observations; pre-invocation failures stay distinct from stream failures. Explicit stream end without a terminal result does not invent one.

Each protocol adapter owns its projection, wire conversion, authentication defaults, setup/retry behavior and usage/cost interpretation. Preserve the two-pass tool-ID projection, including omitted failed assistants and retained tool details. Partial arguments use repair/best-effort parsing during streaming and finalization; execution validation is later and gives corrective field/name/received-value feedback. Direct payload/response callback failures fail the invocation, except response-session response callback failures enter that adapter's setup-retry path; when an adapter invokes the response callback, it otherwise runs before the first event/body processing, not for every intermediate setup retry. The public and cloud content-generation adapters do not invoke the response callback. Header layers use ordered case-insensitive replacement, including repeated names in one layer, except for content-generation and response-session requests. Content-generation headers first merge exact-spelled keys across their layers, then normalize and combine surviving case variants; a same-spelled later value replaces an earlier value before validation. The response-session adapter calls the response callback for every actual response before status/body processing and combines differently-cased initial model headers while additional headers replace, validating initial values even when later overwritten.

Ship every catalog provider and all nine bundled real protocol adapters: message, chat-completion, conversation, response, cloud response, response-session, content-generation, cloud content-generation and cloud conversation streams. Wire identifiers and accepted provider/model literals remain unchanged. Include the opt-in scripted simulator with builders, chunking, pacing, usage/cache simulation, request-observing factories and pre-aborted queue consumption. Ship every bundled OAuth/device/refresh flow, registry operation and dedicated model-account command. Generic registration alone does not satisfy bundled-provider delivery.

Keep the complete offline catalog and developer generator. Do not add runtime catalog refresh. Preserve raw versus simple budgets/options and provider-specific payload defaults, omission rules, retry parsing, proxy/environment behavior, stream end-marker/EOF handling, overflow classification and accounting. A clean chat response can retain its initial stop outcome without a finish-reason field; other protocols retain their own completion rules. Successful measured usage can satisfy the public overflow predicate. Test all protocols with synthetic wire fixtures; paid/live calls require separate authorization.

## R02 — Agent ownership and execution

Provide inert construction, explicit replaceable instructions/model/history/tools/callbacks, reset, empty/batched input, active signal, pending tools, last error, streaming state, steering and follow-up modes. Keep independently owned running work when a prompt handle is dropped. Default transport is auto. Low-level Agent dispatch reduces state and invokes the first live listener synchronously, including tool progress callbacks, then awaits listeners in live subscription order with the active signal; their promises are part of run settlement and waitForIdle. Add/remove during dispatch can affect that dispatch. The application session has a separate synchronous, fire-and-forget public-listener interface; do not conflate the two layers.

Provide raw loop and continuation entry points, the stateful owner and the proxy stream client. Preserve before/after tool boundaries, source-ordered results, parallel/sequential execution choices, preparation before validation, intercepted errors, progress, termination and remaining-tool handling. Ordinary fallible callback errors follow the synthetic error-assistant/history/end-event path before runtime flags clear. Do not turn every Rust panic into a promised recoverable callback error.

## R03 — Credentials, settings and local catalog

Credentials maintain a tolerant accepted snapshot until explicit reload, ordered provider metadata and drainable errors. Authentication availability is distinct from stored/login presentation. Mutations update accepted memory even if persistence fails. Preserve current stored token forms, branch-specific expiry/fallback and refresh locking. Thrown refresh reloads and blocks ambient fallback unless a newly valid stored token exists; a nonthrowing empty refresh can continue normal fallback. Cached secret commands use the whole command string and inherited cwd; per-request configured keys/headers execute fresh and can throw. Listing metadata executes neither.

Settings retain user/project scopes, runtime overrides, established names/defaults, a top-level merge with one nested object spread, immediate cache publication, queued writes, flush and error drain. Reload accepts each healthy scope independently, retains a failed scope and rebuilds effective values without preserving runtime overrides. Use Rust `std::fs::File` locking as-is for native storage mutual exclusion. Keep application read/callback/write ordering, explicit application retry loops, permissions, memory publication and failure handling. Do not reproduce lock-directory leases, heartbeats, mtime probing or library retry internals.

Settings persistence uses explicit write intent: native `SettingsStorage::with_write_lock` creates the parent as needed and holds the stable sidecar lock before reading, through its single callback and optional write, including first creation. `Ok(None)` and callback failure may leave a parent/sidecar but never create the settings file. Concurrent independent manager edits therefore merge against fresh text under exclusion. The legacy `with_lock` remains noncreating for missing-file reads and calls its callback before locking a first replacement; raw callers needing concurrent first-write exclusion choose write intent. Supplied adapters default to their existing `with_lock` contract; native decorators forward write intent to retain native exclusion.

Workspace preference saves use the same captured-edit queue at literal nested
key paths. `SettingsManager::set_global_key_path(field, keys, value)` publishes
immediately and merges only the addressed leaf into fresh storage under the
existing write lock. `None` removes that leaf without creating absent parents;
an empty key path addresses the whole field. Existing whole-field and object-key
setters retain replacement semantics through this same path representation.
`set_global_key_paths` publishes several such sets or removals as one cache
change and one queued write; the single-path method delegates to it.
Layered scope merging remains one object level; nested write intent does not
change read precedence or introduce another writer.

Local model files use partial nested overrides, validated supplied limits and zero defaults for omitted price fields. Dynamic provider replacement/removal is separate and restores underlying catalog entries. Selection preserves distinct command-line, exact-reference and scope matching: saved model within scope first, otherwise first scoped model; catalog-order exact matches where specified; uniqueness only for the exact-reference operation; provider-prefix/raw-ID/suffix/alias/date/glob ordering and warnings. Internal mode has no manifest, origins or locked values; [manifest mode](manifest-mode.md) adds them only when settings name a manifest.

The model catalog owns private synchronization; registration, removal and refresh use shared receivers. Release catalog, configuration and descriptor guards before supplied file, credential, OAuth, configured-value or display-name effects, global registration/reset, awaits and callback retirement. Successful transformations publish exactly their selected list; failures retain prior effects without retaining the failed incoming registration. Loading publishes only after its local transformation chain succeeds. Refresh traverses live retained insertion order, including callback additions; replacement keeps its position and removal followed by insertion moves to the end. Request authentication retains the early provider-source entry while sampling model sources later; provider-only fallback is sampled after credentials. Load diagnostics are owned observations. Native file effects are thread-safe; browser effects may remain local. Do not expose a state guard or require an outer lock.

## R04 — Session trees, persistence and exports

Native sessions use the established JSONL files, project-path encoding, identifiers, headers, versions and append-only tree entries. Also support explicit memory sessions. Keep one in-memory owner for entries, branch position, labels and resolved context. Append changes memory before persistence; new sessions delay initial file publication until the established assistant condition. Do not add transactional batch admission, uncertain-write disablement or close protocols. Preserve malformed/truncated-file handling, existing conversions, discovery, modification/activity metadata, continue/open/fork/clone/import and progress callbacks.

Support branch traversal, names/labels, custom entries/messages, tool pairing, recorded model/thinking changes, compaction context, abandoned-branch summaries, usage/statistics and selected-branch JSONL export. Compaction preserves cut-point/token-estimate/image/tool-pair rules, exact prompts, retained-budget behavior and event/queue ordering. Import is current-session interchange, not a general migration framework.

Deliver the complete self-contained offline session viewer: escaped data, tree navigation, filtering/search, deep links, collapsed details, syntax/Markdown/image/custom-tool rendering, statistics and JSONL download. Implement its behavior in Rust with a light-DOM browser viewer and embedded assets; opening the exported file requires neither a network service nor consumer compilation. The document serializer accepts data, render capabilities and a prebuilt viewer asset bundle; it does not import a frontend. Shared retained occurrences, stable active-first ordering, physical/visible tree projection, content extraction/search and raw-record ancestry live in `maestro-session::export_html::template`, reached by frontends through `maestro-app::export_html`. Filter, search, fold and selection state remain frontend-owned. The viewer itself belongs to the web frontend and calls the application's read-only session-view interface. Its entry values prepare raw operands before any browser effect: a falsy timestamp keeps the native locale call unperformed, and the prompt's skill block is parsed by the one application owner that native transcripts and the browser viewer share, available to browser builds rather than a second parser. Build packaging injects its bundle into the serializer, avoiding an application-to-frontend dependency. User-triggered online sharing is a separate operation with an owner-chosen destination.

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

Use three jobs: maestro-extensions governs semantics; maestro-extensions-wasmtime executes components through those ports; maestro-extensions-wasm provides guest authoring and the canonical WIT. The guest's allowed internal dependencies are maestro-request (message and model-request records), maestro-entry (session interchange records), maestro-tool (tool interchange records) and maestro-json (JSON boundary semantics); it never imports native session, tool or catalog runtimes. The runtime adapter depends toward the domain, with the optional native path utility, and is wired by the executable. The host reads shared WIT build input without linking the guest crate. Real host capabilities, event/discovery conformance, UI lifetimes, SDK/examples/docs and platform gates remain acceptance, not claims proven by ABI probes.

Event payloads and results cross the existing generic native-async export as plain JSON beside their typed capability envelope. Use the shared records' existing serde_json representation, not duplicate guest models or a second serializer format. Finite floating-point numbers roundtrip exactly, including signed zero. Ordinary host serialization turns nonfinite numbers into null; this does not promise that null decodes into a required number. An extension writing Infinity or NaN receives a clear error before its edited event or result is encoded. This explicit error is the accepted difference from retaining the nonfinite value until serialization. No bit-encoded numbers, numeric transport wrappers or compatibility decoder are permitted. Shared records retain their existing optional-field semantics without added Presence wrappers; unshared event fields still preserve their meaningful missing/null distinctions. Preserve separate event-edit and callback-result outcomes when either fails.

Use Wasmtime/wasmtime-wasi at unmodified upstream commit `ffff2e2db24a291f02429f123aba1631fa8eea67` and the Rust SDK bindings for one WebAssembly component system. Replace this temporary Git pin with the first published release containing its queued-event delivery fix. The application extension world uses WASI 0.3 native async, with WASI 0.2 interfaces needed by Rust standard-library basics. Support an authoring language when its toolchain emits the required component contract. Rust is the initial qualified authoring path; JavaScript/TypeScript authoring is unavailable until its tooling supports that contract. Do not add a polling workaround, a separate legacy source loader, a second host or an out-of-process extension protocol. Runtime JIT is allowed; install/use never invokes Rust/Cargo compilation.

The component SDK preserves every host capability: ordered live events/hooks, transforming and cancelling operations, tool schemas/validation/progress/results, commands and command-only context methods, flags, shortcuts, providers/OAuth/streams, source discovery, session queries/entries, messaging, exec/network/files, live getters, themes, renderers, custom editor/components, focusable overlays, status/footer/header/widgets and reload. UI render/input/invalidate calls remain synchronous where their callers require it; asynchronous work uses native async and owned resources, not JSON-only replies. UI-entering interactive execution and its synchronous callbacks share the caller-owned local executor. Independent native I/O and the default embedding SDK retain thread-safe execution; this adds no scheduler or runtime setting. Use shape A capability interfaces and typed resources; register handlers one by one so the host retains per-handler semantics. Messages, entries, provider payloads and tool arguments use session JSON losslessly. The domain owns the existing ExtensionRuntime action record, factory facade and local contexts. Only the Wasmtime adapter owns its engine/linker/bindings, one component instance per extension, stream pipes and trap diagnostics. Typed requests connect its thread-safe Store to the caller's local domain/UI owners; synchronous callbacks pump requests on the original local receiver. Callback emission retains the bus's emit-time listener snapshot in registration order. The guest invokes its own listeners locally and waits for each consecutive host-listener segment through a synchronous import before continuing; nested emissions repeat that walk. Each own listener keeps an independent native callback frame, error destination and tail boundary. Foreign instances retain the callback-return delivery rule. Internal callback exports are async-typed so synchronous host requests may wait on the dedicated Store thread; guest author callbacks and callback emission remain synchronous. This adds no second receiver or Store lifecycle. The guest owns one canonical WIT source checked against host code generation. Optional command and compaction options retain absence separately from a present empty record across that interface; model reads return owned values. Network operations use the WIT http interface backed by host reqwest, not a second WASI HTTP library. The shared interface hides runtime-library types.

Extensions receive the user's full effective permissions through host operations. Add no capability grants, restriction policy, fuel, memory limits or deadlines. Preserve registration-specific collision behavior: same-extension replacement, cross-extension rules and individually diagnosed provider failures; there is no whole-owner atomic validation policy. Hooks iterate current collections in load/registration order. Presence of UI does not imply every mode implements the same methods, and every disposal/reload does not automatically settle dialogs.

Application session operations own replacement ordering and context rebinding. Command contexts select bound command operations when called; admitted command futures retain their selected operation through completion. Registration retains its per-handler behavior without a separate generation-publication controller. Reconstruct new guest state through the same session state/hooks, not heap migration. Validate stale/foreign/disposed handles before Wasm entry; preserve operation-specific in-flight cancellation and reentrancy. Initially retain compiled components only in a process-local identity-bound cache; never deserialize native bytes from packages, users or disk. Compile the original component on a miss. A persistent native cache needs a separate integrity proof: full-user extensions can modify same-user files, so filesystem ownership alone is not a trust boundary. Stale or not-yet-bound context calls return recoverable, catchable errors, not traps; one caught stale call must not terminate another task. Traps are reserved for actual component faults and do not justify calling a poisoned instance again.

Release qualification must exercise the complete Rust native-async SDK with actual extensions, pending provider streams and host I/O; full transform/veto/cancellation/progress semantics; reload during old commands; failed activation and partial provider errors; widget focus/theme/Unicode/invalidation/disposal; distinct-component memory and repeated reload cleanup; and platform/license checks. Preliminary registration/clock probes do not prove the complete SDK.

## R08 — Application, SDK and lifecycle

`maestro-app` is the one application interface and the embedding SDK. It composes model/agent/session/credential/catalog/resource/tool/extension/package services through ports. It owns prompt delivery, steering/follow-up publication, events, model/thinking selection, compaction, visible retries, direct-shell transcript timing, navigation/import/export, source reload and session replacement. Frontends select presentation modes, not alternate business rules. Browser-specific tool/artifact/storage policy also lives here, using browser adapters supplied by the web frontend.

Preserve lifecycle-specific veto/preparation/shutdown/disposal/rebinding order. There is no universal abort-and-idle gate or rollback after teardown. Interactive reload refuses streaming/compaction; direct SDK reload preserves its distinct lifetime. Application retries classify nonempty assistant error text after excluding overflow; do not add unconditional authentication/billing/quota exclusions. Aborted outcomes do not retry. Keep application retries separate from transport setup retries. Preserve queue recovery, active context replacement, event bus semantics and costs added at the defined message boundary.

The browser build uses the same application crate with capability-selected native/web adapters. Native process, file and component-engine implementations are not linked into a browser just because the application exports their contracts. The browser surface does not acquire native operations it never exposed. This target separation requires an actual wasm build/smoke test, not a conditional-compilation claim alone.

## R09 — Terminal toolkit and interactive frontend

Provide a reusable terminal library with no UI framework and internal dependencies limited to shared cancellation, shared text operations and the optional path utility, as defined in the [crate graph](#crates-and-delivery-order). Port the owned retained-line frame writer and all twelve components: box, cancellable loader, editor, image, input, loader, markdown, select list, settings list, spacer, text and truncated text. Preserve normal scrollback, differential redraw/resize/clear/purge, synchronized updates, raw OSC8/image escapes and hardware cursor/IME location. Use the terminal adapter crate through the terminal port, which on Unix drives the standard descriptors directly; inject a clock for the 16 ms render throttle. Preserve async raw input, Kitty keyboard, bracketed paste and COLUMNS/LINES fallback. The standalone virtual-terminal harness depends only on the toolkit, and the command-line frontend is its only development consumer, for the selector presentation effects. Require the complete source scenarios plus a real Linux tmux cross-check.

Preserve synchronous component render/input/invalidate, retained component state, focus routing, live input listeners, overlay positioning/z-order/capture/disposal, background/style boundaries and custom editors. Port raw/Kitty/legacy key decoding, release filtering, fragmented escapes and bracketed paste. The editor preserves actions, grapheme movement, sticky visual-column movement, history, atomic paste markers, kill/yank, undo, completion cancellation, fuzzy ranking and file/slash completions using native segmentation and width libraries with Rust-valid string boundaries under the common contract. Do not introduce a UTF-16 string/event substrate. Port every widget, Markdown block/inline behavior, image protocol, theme/palette, watcher and image fallback. Highlighting belongs to the shared theme library and uses syntect 5.3.0 with two-face 0.5.2+bat-0.26.1, defaults disabled and syntect-fancy enabled, never the onig C library. Keep the theme format and its nine syntax color keys, mapped from scopes. Every terminal/browser/viewer consumer uses that interface, with an injected callback keeping the terminal toolkit independent of highlighting libraries. In the terminal, resolve only explicit language names/extensions and data-defined aliases in the selected syntax set; disambiguate collisions and never auto-detect. Absent/fancy-excluded languages using the terminal plain fallback are recorded accepted differences. The terminal highlight path admits only the labels its own language table names, aliases included: an unnamed or empty label colors every row with mdCodeBlock through the published theme and fails without one, while a named label whose grammar is absent keeps that color through the published theme, returns its rows unstyled without one and does not fail. Preserve the distinct direct-helper raw-line error fallback and the Markdown callback's mdCodeBlock-colored error fallback. Token colors may differ; no old-engine token-parity requirement remains.

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

Preserve omitted/null field distinctions, empty last-assistant data `{}`, parse failures without IDs and the actual invalid-value/failure boundary; promise neither arbitrary-JSON survival nor universal shutdown drain. RPC UI has exactly the nine established wire methods: `select`, `confirm`, `input`, `editor`, `notify`, `setStatus`, `setWidget`, `setTitle`, `set_editor_text`; unsupported component operations preserve their documented no-op/absent behavior. Dialog response, supplied cancellation and supported caller timeout retain their individual ownership. The stream has exactly 17 session event types plus extension_error: 18 total. Preserve every source flag, --theme/--no-themes and all 13 short aliases. The newer max thinking level is not in this baseline. Also deliver the typed subprocess client with process/stderr/correlation ownership and its own deadlines; the server alone is not client delivery. The client is `maestro-app::rpc_client`, beside the shared session command and response records; it drives its child through `maestro-process`, and `maestro-rpc` stays the server's I/O adapter.

## R11 — Browser frontend, documents and sandbox tools

### Browser problem statement

The browser application needs selectable, searchable and accessible page content with native text entry and independently replaceable views. A renderer-specific geometry interface must not constrain chat, documents, installed package views or the control-plane shell.

### Browser solution

Deliver an embeddable Rust browser application using `web-sys`, `js-sys` and Rust-authored light-DOM Custom Elements through `custom-element = "=0.1.5"`. Build page content in the browser. Use native HTML controls, CSS layout and native SVG where appropriate. Do not introduce a general application framework or server rendering.

Preserve chat, the editor, attachments, artifacts, streaming, reader-follow behavior, abort/steer/follow-up, model/thinking controls, localized text, copy/download/links and embedding callbacks. The same component layer supports Home, Chat, Catalog, Graph, Workspaces, Docs, Settings and personal records. The desktop host loads the same web bundle; it does not supply a second renderer.

The model library owns replaceable, library-type-free WebSocket effects beside its HTTP effects. The native adapter owns the shared socket-upgrade primitive; response-session caching, cancellation and lifetime policy remain provider-owned. The browser frontend supplies its browser socket adapter through the application facade, reusing its event-subscription owner. One-shot model discovery owns authentication, RPC admission and connection release, without a retry policy or cache.

Provide the browser's named stores, metadata/indexes, settings/provider keys/custom providers, session listing/deletion, quota/persistence request behavior and artifact reconstruction through IndexedDB. Preserve all supported local-server discovery, authentication/proxy URL handling, key dialogs/tests and provider settings. Browser server discovery is not the excluded newer core catalog-refresh feature.

Accept the established URL/File/Blob/buffer inputs and text/image/PDF/DOCX/PPTX/XLSX/XLS formats. Preserve original bytes, MIME/name/size, extraction wrappers/order/whitespace, previews, cancellation and errors. PDF attachments use owned text-show traversal with registry hayro-interpret 0.8.0 font decoding and hayro 0.8.0 page rendering for the first-page preview; PDF artifacts render all pages at the defined scale with cleanup. Keep application wrappers, fit-160px preview and 1.5 artifact/overlay scales, errors and cleanup exact. PDF extraction and rasterization retain the approved native engine’s text segmentation, Type3 text, bidirectional ordering and font-rendering differences; application wrapping, page order, scales, failure categories and owned cleanup remain exact. Do not use pdf-extract. DOCX preview preserves pagination, fonts, headers, footers and notes. Spreadsheet preview preserves sheets, formatted cells, styles and tab switching. HTML/SVG/image/Markdown/text/code artifacts preserve preview/source/download and safe rendering behavior. Browser Markdown also preserves inline and display math for dollar and backslash delimiters, HTML math layout/fonts, parse-error spans and fallback text. Use katex-rs 0.3.0 beside the Markdown consumer with output="html", throwOnError=false and delimiter-specific displayMode. Ship matching KaTeX 0.18.5 CSS/fonts as hashed data through the DOM. Reuse the reference math inputs for named checks of complete expressions, inline errors and browser layout. Formulas match the reference's glyphs, layout and tooltip content. Internal markup, class names, element structure and byte or pixel identity may differ only where not user-visible. A visible difference is a defect to fix in the owning Rust implementation, not an accepted result. Fix defects in the application wrapper before landing; do not add MathML, auto-render extensions or math behavior to terminal/export paths that never invoke it. Libraries must qualify these contracts; extraction-only substitutes do not satisfy document preview.

The application owns browser attachment values and their live shared handles.
User messages, runtime attachment lists, tiles, overlays, document previews and
downloads retain the same `Arc<RwLock<Attachment>>` members. Removing a parent
collection does not detach or freeze a retained member. Model projection and
runtime publication snapshot values at their existing boundaries; release all
collection and member guards before invoking caller-supplied value adapters.
Serialization keeps plain attachment objects, optional fields, order and repeated
members, without persisting pointer identity or introducing a second record schema.
Assistant rows retain the application's existing shared assistant-message handle.

Shared XLSX formula displacement belongs solely to the application's existing
`browser::ssf::read::formulas` module. Classify maximal runs between operator
characters, whitespace and operand punctuation; every other character belongs to
a run, including combining marks, `#`, `~`, `\` and `?`. Preserve quoted strings/sheet
names (including doubled delimiters) and balanced bracket spans first. Then classify
runs in order: leading `#` errors, sheet qualifiers ending in `!`, functions directly
followed by `(`, exact bounded coordinates, coordinates with one trailing spill `#`,
and otherwise opaque names/literals. Whitespace before `(` remains an intersection.
A qualifier may contain `run:run!` unless its first run immediately follows `!`;
this resolves the ambiguous colon as a 3-D sheet qualifier, so `A1:Sheet2!B2` protects
`A1:Sheet2!` too. Coordinates match case-insensitively and retain their column case
when advanced. Whole-axis pairs remain unchanged. Preserve absolute axes and
byte-identical zero/fixed displacement. Render every advanced coordinate, including
output beyond the input grid, with the existing address owner rather than silently
retaining the original reference.
Add no grammar recognizer, evaluator, parser API, dependency, budget or diagnostics.
Unterminated quotes/brackets protect the remainder. SpreadsheetML/SYLK own R1C1
conversion; XLSX templates do not. The single operand-blind displacement correction
protects identities/text, Unicode boundaries, lowercase references and qualifiers;
formula decoding, sheet-local indices, cached values, arrays, CSV serialization
and attachment extraction keep their existing owners.

Recreate DOM iframe sandboxing, injected runtime providers, postMessage bridges/routing, initialization/ready/run/cleanup lifetimes, console capture, attachments, downloads and artifact access from Rust. The browser tool intentionally executes user-supplied JavaScript in its browser sandbox; that is application behavior, not a second extension host. Keep the document-extraction tool, the JavaScript REPL tool (`javascript_repl`, with `title` and `code` parameters), and artifact commands/prompts. The calculation-result renderer remains rendering only, not an executable calculation tool. Do not use the stronger-looking native extension permission model to alter browser sandbox observations.

Provide the browser embedding example, Rust build/asset pipeline, supplied transcript fixtures, developer prompt measurement, localization and browser smoke/interaction tests. There were no automated browser test-suffix files in the inventory; therefore new source-derived browser tests are required, not optional.

The sandbox's already-injected in-iframe runtime remains a build-time embedded JavaScript asset. Its Rust host preserves srcdoc flags, router, console, timeout/abort and source cleanup, with sender validation and per-sandbox external-link listener cleanup corrected at the owning adapter. This is not a JavaScript UI or an extension host; a busy loop may block the parent process and timers, so no hard-interruption guarantee is added.

Browser smoke covers Firefox/Safari, real touch/IME and real HiDPI behavior, not only an emulated fallback. The author build preserves whole brand fonts with their notices and a symbol fallback, and runs pinned wasm-opt, recording bundle/glyph/behavior evidence without a new numeric budget; consumers never build these assets. An isolated editable author fixture qualifies the shared build path before product activation; it is not a distributable application or complete embedding example.

### Browser user stories

#### Chat and reading

1. As a reader, I can find and select transcript and document text using the browser, including offscreen transcript text.
2. As a chat user, I can compose with my input method, attach supported files, stream responses, stop work and keep reading when I scroll away.
3. As an embedder, I can replace tool and message rendering without converting my in-process callback-bearing values to JSON.
4. As a document user, I can preview, inspect, copy, download and export every supported artifact without exposing the host to generated scripts.
5. As an offline reader, I can open one exported file, navigate branches, search, inspect details and download JSONL without a server.

#### Control plane and packages

1. As a user, I can navigate one responsive shell and retain the approved Home card layout and settings.
2. As a package author, I can add data/template views or separately built browser views without rebuilding the application.
3. As a catalog user, I can inspect sources, locks, problems and relations, with edits going through the real owner.
4. As a workspace user, I can inspect and control authorized panes through their existing terminal/session owners.
5. As a signed-in identity, I receive only my own layout, inbox, stars, recents and subscriptions.

### Browser DOM components

Each component owns its root and connected view state. It mounts native nodes once, updates the affected properties/text/children and disconnects its subscriptions when detached. Reconnection attaches exactly one live binding. Keep class-definition callbacks for the lifetime of the registered class; an element's disconnected view is a different lifetime. The layer supplies only repeated, proven element/lifecycle operations, not a template compiler, virtual DOM, reactive dependency graph or global scheduler.

Keep file-mapped component names and exports in Rust casing. Small function components may return native nodes without defining a custom tag. Components with custom-element lifecycles use autonomous tags and light DOM. Use explicit properties/update operations for structured values, not attribute-string serialization of the whole model. Preserve focus, edited input, selection and unaffected row identity across unrelated changes. Ordinary buttons, inputs, textareas, headings, navigation, links and dialogs retain native semantics and accessible names.

The current brand projection supplies semantic CSS values once. Keep existing Markdown parsing, safe HTML preparation, syntax highlighting, math output/fonts, formatting, document decoding and storage owners where they already belong. Untrusted source text is never a component template or host-DOM executable module. Intentional sandbox execution remains separate from host markup rendering.

The stable transcript and streaming subtree are separate. Pair tool results by exact call identifier, using the last result for a duplicate identifier. Preserve emitted row order and positional keys. Filter persistence-only artifact records before custom-message dispatch; registered roles otherwise take precedence over built-ins. Keep all searchable transcript text attached. Batch only streaming publication at the next animation frame using the existing callback owner; clear synchronously and retire only that update. A subsequent response must publish on its first frame. No JSON cloning is required for change detection.

Reader-follow uses the native scroll element and fractional CSS observations. Disable only for nonzero upward movement with distance from bottom strictly greater than 50 px; resume strictly below 10 px. Equality does not cross either boundary. A shrinking editor viewport updates remembered height without changing follow state or remembered top. Follow content growth only when enabled; explicit caller override remains available.

### Three registration interfaces

Keep three separate domain interfaces:

| Interface | Identity and behavior |
|---|---|
| Tool renderers | Open tool-name map with explicit replacement, built-in registration, unknown-tool fallback and forced JSON presentation. Rendering receives original params, optional result and streaming state; content retains the custom-card choice. |
| Message renderers | Open role-name map with explicit replacement and custom-before-built-in selection, except persistence-only artifact records are never rendered. Rendering is independent of message-to-model conversion. |
| Shell contributions | Package/session-owned pages, Home cards, named child slots, route references and declared host API references. The shell projects enabled contributions into navigation, headers, tabs and content. Contribution kinds and identifiers are extensible data. |

Share registration lifetime handling and adapters where necessary, not one universal registry. Same-module Rust callers retain typed operands and callbacks, including non-serializable values. Separate browser modules exchange explicit serializable values and standard DOM events/host calls. Rust trait objects, type identifiers, reference-counted handles and heap pointers are not an inter-module interface.

### Runtime browser modules

Use the package manifest's existing `maestro.contributions` resource. Data/template pages and cards remain supported. Add a renderer reference and optional browser-module descriptor to those contribution records; do not add another plugin manifest, installer or package-resolution path.

The contribution record supplies its stable package/contribution identity, attachment-point reference, consumed/provided data contract, required host API references and route references when routable. A browser module descriptor locates a prepared module entry, its matching WebAssembly/binding assets and its registration export within the resolved package. The entry initializes the matched browser module and registers its elements and contributions against the supplied host interface. Installation/use never compiles Rust. Backend extension artifacts are not browser module artifacts.

Package resolution owns ordering, provenance, enabled state, manifest locks and reload. Package pages work without a running chat; session-only contributions keep their session lifetime. The application's resource loader reads the accepted descriptors on each reload, and the shell exposes them and their operations through the existing protocol. The frontend loads only accepted enabled modules and mounts their declared elements at compatible slots. A module may supply child slots and narrow host API implementations. Required missing APIs, incompatible inputs, failed initialization and conflicting route/slot attachment are visible contribution failures, not silent empty pages. Resolve declared API references only; do not expose application internals or a general service locator.

A registration invocation returns owned registrations and mounted-view cleanup. Disabling unmounts views and removes only registrations still owned by that invocation; it cannot erase a later explicit replacement. Tool/role maps retain their replacement semantics; package contribution collision/order rules remain package-owned. Failed loading releases acquired registrations/listeners without claiming to unload executable code.

Custom-element definitions cannot be undefined or redefined. Use unique element names for new definitions, with stable contribution/renderer aliases. A changed definition uses a fresh element name or reloads the page. Do not promise safe unloading of executed WebAssembly. Keep the class constructor closure alive after individual views disconnect. Reuse existing listener/frame/timer ownership, not another event bus, watch loop or retry scheduler.

### Trust split

Built-ins and vetted installed UI modules may run as host light-DOM elements. They are trusted page code: they can read the host DOM and share page authority. Manifest API declarations are composition contracts, not a sandbox or an access-control boundary.

Generated and untrusted UI stays in isolated sandboxed iframes using the existing runtime-provider interface, without same-origin permission. Match each message to its registered frame source; an origin string or guessed sandbox identifier is insufficient. Preserve permitted attachment/artifact access, console, downloads and actions through explicit providers. Never pass credentials, arbitrary desktop invocation, filesystem handles or the protocol socket into an isolated frame. Main-page find covers host transcript and reading pages; isolated content remains separately accessible inside its frame.

Use one authentication interface, with the local one-time-key adapter now and a hosted adapter later. Enforce origin/authentication/authorization at the command owner, including module assets, data and terminal actions. Hiding navigation is not authorization. Home layout, inbox, stars and recents are server-side per-identity records; a browser-local preference is not their authoritative store. Theme, language and sidebar preference retain their existing local behavior. Do not introduce an identity provider library or a hosted deployment as part of rendering replacement.

### Control-plane composition

The shell owns navigation, route resolution, page headers/tabs and Home composition. Catalog pages/cards read the one graph through the application. Kinds and relations are data; actions reach their original owner and respect manifest locks. Read graph sources on demand without another graph store, cache or watcher. Docs reuse safe Markdown rendering and in-app links/heading anchors. Search reuses existing matching/session search; this is not a new search engine.

The relationship page uses the existing diagram layout and native SVG with DOM text labels. Workspace pages mount a terminal-view element over the existing terminal screen/input protocol; the terminal engine and daemon retain emulation, ownership and control arbitration. Do not add a browser terminal emulator or an immediate-mode painter.

Home retains full drag/resize and per-card settings with keyboard-accessible operations. The selected page/card/route can be supplied by a built-in element, a separately built trusted module or an isolated adapter without changing its application caller. Later collaboration and IDE views use this seam but retain their own scope, protocol and permission designs.

### Browser crate graph

The web frontend remains one library, `maestro-web`, with required `maestro-app` and optional-used `maestro-theme` edges. No new workspace UI crate or frontend-to-frontend edge is introduced. The DOM component layer, browser-module loader, three renderer/contribution adapters, shell and offline viewer remain inside that frontend.

Package descriptor resolution remains `maestro-packages` work, coordinated by `maestro-app`; domain-neutral browser contribution records cross `maestro-protocol`. Native package code is not linked into the browser merely to expose a descriptor. Graph, workspace, terminal, identity and per-person record operations are reached through the application/protocol interfaces, never by importing a sibling frontend or a native service into a browser view. The daemon must not gain a reverse dependency on the application; its composed operation handlers are injected through the existing interface.

The executable and desktop roots only wire adapters. Export serialization still accepts a prepared viewer bundle; the serializer never imports the web frontend. Update dependency-policy tests, metadata checks and documentation together if an owning prerequisite changes an allowed edge. This amendment itself adds no internal Cargo edge and creates no placeholder crate.

Pin the browser bridge and bindings exactly: `custom-element = "=0.1.5"`, `web-sys = "=0.3.106"`, `js-sys = "=0.3.106"`, `wasm-bindgen = "=0.2.129"` and `wasm-bindgen-futures = "=0.4.79"`; enable only used browser interfaces. Gate new executable browser dependencies and browser effects to the browser target while retaining any browser declaration dependency needed for native type-checking of public interfaces. The fixed subclass shim and generated bindings are interop, not an authored JavaScript application. Retain one native feature unifier and qualify the complete resolved dependency/license closure.

### Removed implementation

Remove immediate-mode browser painting and all its dependencies, default test fonts, textures, paint commands, canvas contexts, layer transforms, hit-rectangle apertures, native-control slots, placement/measurement/publication transactions, repaint coupling, rasterized UI glyphs and canvas paint probes. Do not retain color, geometry or identity types from the retired renderer. CSS, native controls, native SVG and DOM ownership replace them.

Keep only framework-independent code already in its appropriate form, with a per-file justification. Rewrite or delete the remaining wrappers even when they compile without a direct renderer import. Preserve supported document image/raster outputs where they are content, not a UI painter. Remove stale source, test, docs, lock, feature-unifier and CI/font-exception references. Prepared author fixtures and distributions use the same DOM model.

### Browser testing decisions

Test through the real renderer/contribution interfaces and application callers. The initial qualification builds two independent browser modules, registers a new renderer and a new page/card/route after host startup without rebuilding the host, and replays identical streaming inputs through the built-in and module adapters in Chromium. It also exercises the isolated adapter and same-module non-serializable operands. Record actual initialization, reentrancy, class/view lifetime, disable/reload/replacement and cleanup observations before accepting the chosen bridge.

Use the one approved Chromium job and existing Rust browser exports, alongside native behavioral tests. Test accessible roles/names, keyboard/focus, selection/find, composition input, scrolling, copy/download, sandbox source checks, standalone offline export and two-identity record/subscription isolation. Measure DOM mutations, browser frame time, initial load, repeated mount/unmount memory and complete raw/compressed distribution size. No new numeric performance threshold is introduced. Require the existing meaningful 80% line-coverage standard with explicit covered/eligible counts; runtime success alone is not a coverage measurement.

Keep the previous runnable path until each replacement passes, then switch that surface and remove its old implementation. Temporary co-existence during replacement is not a permanent hybrid or a published second component interface. The final removal verifies the full dependency closure and all authored files contain no retired renderer-family dependency or reference. Browser/device smoke obligations remain separate from headless Chromium qualification.

### Browser out of scope

Server rendering/hydration; another application framework; a new package manager or protocol; background reconnect/retry/watch machinery; new graph/layout or terminal-emulation libraries; hosted sign-in implementation; collaborative editing and the browser IDE. These are not implicit consequences of adopting native page elements.

## R12 — Documentation, examples, repository tooling and release

Development commands (offline test launcher, source launchers, commit hook, asset copying, browser smoke check) live in `maestro-tooling`, which is never shipped, has wiring-only binaries and exposes its library to integration tests.

Browser-author preparation and check/build/watch dispatch share that library's command operation. The editable fixture is an independent workspace with its own reviewed adjacent lock and no native path dependencies or build script. A locked native `maestro-theme` example exports presentation files and authored/canonical pack/mark directory observations; the fixture includes those files through `MAESTRO_BROWSER_INPUTS`. Tooling prepares route-specific inputs and absolute-path Trunk JSON configuration under `target/maestro-browser/.author/{library,example,debug}/`, outside replaceable distributions. Trunk alone watches and serves; its pre-build hook repeats the same locked native export. Authored hooks keep their authored working directory, arguments, platform overrides, inherited environment and failure status through the existing development command. Authored watch and ignore paths resolve against that directory. Startup watch roots retain authored roots and include the fixture, default identity and observed pack/mark directories; the generated preparation tree is ignored, and selecting dependencies outside the watched roots requires restarting.

Root preparation, direct fixture checks and Trunk compilation use locked Cargo. Only Trunk's `CARGO` environment points to a wiring-only tooling adapter that adds `--locked` to metadata while preserving literal arguments, diagnostics and exit status; other invocations pass through. Bootstrap the native exporter and adapter from source using the root lock, with artifact locations obtained from locked metadata. Authored source, manifests, locks, identity and borrowed Cargo configuration are never rewritten by preparation or builds. Explicit check formatting alone may format authored Rust. Writes are restricted to Cargo outputs/caches, route preparation and selected distributions; preview needs no author tools and clean removes only the selected distribution.

Declare the independent manifest through shared CI's `additional_locked_manifests`. Provision both locks explicitly before tests; cold-cache qualification vendors locked offline sources into test-owned storage and starts with empty Cargo home/target and directory-source offline configuration. Test setup never repairs authored locks or fetches during the cold phase. Removing the native fixture edge requires one reviewed lock update preserving retained versions; unrelated root dependency changes require no fixture lock maintenance. This is repository build support, not a new runtime dependency, watcher or application.

Maestro does not auto-close issues or pull requests from new contributors; there is no contributor gate.

Deliver current offline API/usage/model/session/settings/tools/extensions/package/terminal/browser/platform/RPC documentation, every executable SDK/extension/demo example and all fixtures/assets. Examples remain examples: an optional subagent, permission guard, sandbox or planner does not make that capability built-in. Port example functionality into Rust component guests and qualify author builds; do not load legacy source examples. Executable code controls behavior and wire acceptance; correct stale documentation and examples to the delivered behavior, listing each correction once. Do not invent historical Maestro releases.

Give every contribution guide, issue template, automation workflow, prompt, ignore file, package manifest, lockfile, compiler/lint/test/build configuration, profiling/analysis tool and packaging script a named Rust-setup counterpart. Use mise to pin just and prek, just recipes as the commands, and prek for the mandatory commit hook: these are the Rust counterparts of npm scripts and the husky hook. Remove unused jaq from mise.toml and mise.lock in the foundation tooling change. Keep YAML issue templates, workflows and prek configuration, including its YAML, TOML and merge-conflict checks. Keep shared CI, not a second build/quality system. Respect dependency-free leaf crates, declared normal/optional/target/build edges, isolated test helpers, frontend independence and wiring-only executable roots through metadata-based convention tests. Shared workflow changes require a separate issue/clone in their repository; the application ticket cannot silently edit that repository.

Default tests run on Linux with deterministic fake providers/clock/terminal/browser/storage adapters and controlled process adapters; required search integration tests also run real rg/fd on temporary fixtures. Restore macOS and Windows qualification before the first release. Every public item has rustdoc; CI/hook documentation builds deny warnings and missing docs. Run `just check`, `just test` and shared `just ci`; do not introduce mutation testing. Hooks format and re-stage only staged files, then check the staged crates and their reverse dependents, falling back to the full workspace check for a workspace-wide input; `just check` and shared CI stay full. Pin Rust via rustup and existing `rust-toolchain.toml`; pin developer tools in mise. Review all direct and transitive distribution licenses. Release notes come from conventional commits; implementation PRs do not edit CHANGELOG.


CI must be equal or stronger step by step. Install rg and fd and require real-binary search tests, not just fakes. As #109 specifies, `just test` remains unwrapped; the separate ported non-LLM test script removes the specified provider credentials and sets the agent's auth file aside, restoring it on exit. Keep strict Clippy, public rustdoc, conventions, full action-SHA pins and merge-queue validation. The full suite includes terminal tests.

Define the caller-driven wasm32-unknown-unknown build gate in foundation tooling and activate it for the model library at its first re-port; every later browser-used crate opts in when created. The first real web application slice builds its actual wasm application and a working embedding example immediately. Full browser application/example builds remain required thereafter; cargo check, native-only compilation and empty examples do not qualify. Release builds cover darwin arm64/x64, linux x64/arm64 and windows x64, with matching notes/assets.

Shared-workflow changes require one separate issue and clone in maestro-rust-workflows. As #109 specifies, `just test` remains unwrapped; the separate ported non-LLM test script removes the specified provider credentials and sets the agent's auth file aside, restoring it on exit. Its initial real-tool and wasm-job release plus caller/ruleset re-pin precede foundation/models acceptance; the caller later supplies its real browser build and its own tag-triggered release workflow; the shared companion supplies only generic configurable checks. Keep the application lane out of that repository. Every mapped CI step needs observed evidence before final qualification; a plan or skipped job is not proof of delivered parity.

## Crates and delivery order

The table declares the enforced domain graph. Its rows and the dependency
descriptions below also permit the [native feature-unifier exception](#native-feature-unifier),
including leaves and exact sets. An em dash means no domain dependencies apart
from the optional foundation utility described below.

The table lists production edges. Guest-authoring examples also have a portable
development-only edge from `maestro-extensions-wasm` to `maestro-listing`.

| Crate | One job | Allowed internal dependencies | Layer |
|---|---|---|---|
| `maestro-text` | implement common text operations (whitespace, lexical comparison, markup escaping, CSS string serialization, locale construction and number formatting) | — | below 0 |
| `maestro-json` | implement JSON boundary semantics (object acceptance, canonical keys, truth, selected-value display, compact and pretty text, JSONL line framing) | — | below 0 |
| `maestro-cancellation` | signal cooperative cancellation | `maestro-listeners` | below 0 |
| `maestro-request` | define shared message and model-request records | `maestro-json` | below 0 |
| `maestro-entry` | define session interchange records | `maestro-request` | below 0 |
| `maestro-tool` | define tool interchange records | `maestro-request` | below 0 |
| `maestro-brand` | project a brand pack | `maestro-text`, `maestro-json` | below 0 |
| `maestro-watch` | supply filesystem notification effects | — | below 0 |
| `maestro-lock` | guard file writes | `maestro-path` | below 0 |
| `maestro-process` | own native child-process lifecycles | — | below 0 |
| `maestro-listing` | list directory entries | — | below 0 |
| `maestro-listeners` | retain callback identities in live registration order | — | below 0 |
| `maestro-pattern` | match file patterns | `maestro-text` | below 0 |
| `maestro-protocol` | define the shared RPC protocol | — | below 0 |
| `maestro-ipc` | connect local sockets | — | below 0 |
| `maestro-markdown` | parse Markdown source | `maestro-text` | below 0 |
| `maestro-extensions-wasm` | provide guest authoring | `maestro-request`, `maestro-entry`, `maestro-tool`, `maestro-json` | 0 |
| `maestro-models` | supply model invocations | `maestro-cancellation`, `maestro-request`, `maestro-text`, `maestro-json`, `maestro-brand`, `maestro-listeners` | 0 |
| `maestro-resources` | discover resources | `maestro-request`, `maestro-pattern`, `maestro-text`, `maestro-json`, `maestro-listing` | 0 |
| `maestro-settings` | own accepted preferences | `maestro-lock`, `maestro-json`, `maestro-resources` | 0 |
| `maestro-test-conventions` | verify workspace structure | `maestro-markdown` | 0 |
| `maestro-test-directory` | own disposable native test directories | — | 0 |
| `maestro-test-fixtures` | share cross-package test fixture operations | — | 0 |
| `maestro-test-process` | run one named test in a checked child process | — | 0 |
| `maestro-test-tracing` | run a test action under a subscriber that sees every callsite | — | 0 |
| `maestro-tooling` | automate repository development | — | 0 |
| `maestro-tui` | render terminal components | `maestro-cancellation`, `maestro-text`, `maestro-process`, `maestro-markdown`, `maestro-listing`, `maestro-resources`, `maestro-listeners` | 0 |
| `maestro-terminal` | emulate terminals | `maestro-text` | 0 |
| `maestro-workspace` | own workspace topology | `maestro-protocol`, `maestro-json`, `maestro-text`, `maestro-lock` | 0 |
| `maestro-detection` | recognize agents in terminals | `maestro-protocol`, `maestro-text`, `maestro-process`, `maestro-lock` | 0 |
| `maestro-agent` | run an agent | `maestro-models`, `maestro-text`, `maestro-tool`, `maestro-listeners`, `maestro-json` | 1 |
| `maestro-credentials` | own accepted credentials | `maestro-models`, `maestro-request`, `maestro-lock`, `maestro-text`, `maestro-process` | 1 |
| `maestro-packages` | manage package sources | `maestro-settings`, `maestro-resources`, `maestro-pattern`, `maestro-text`, `maestro-json`, `maestro-process` | 1 |
| `maestro-test-terminal` | exercise terminal scenarios | `maestro-tui` | 1 |
| `maestro-theme` | resolve presentation styles | `maestro-tui`, `maestro-request`, `maestro-watch`, `maestro-text`, `maestro-json`, `maestro-brand`, `maestro-listing` | 1 |
| `maestro-tui-crossterm` | connect a real terminal | `maestro-tui`, `maestro-text`, `maestro-process`, `maestro-listeners` | 1 |
| `maestro-daemon` | run the workspace daemon | `maestro-protocol`, `maestro-ipc`, `maestro-process`, `maestro-terminal`, `maestro-workspace`, `maestro-detection`, `maestro-settings`, `maestro-tui`, `maestro-json`, `maestro-text` | 1 |
| `maestro-catalog` | resolve the usable model catalog | `maestro-models`, `maestro-credentials`, `maestro-pattern`, `maestro-request`, `maestro-text`, `maestro-json` | 2 |
| `maestro-session` | own conversation history and transcript bytes through native file access | `maestro-models`, `maestro-agent`, `maestro-request`, `maestro-entry`, `maestro-text`, `maestro-json`, `maestro-listing`, `maestro-resources`, `maestro-lock` | 2 |
| `maestro-tools` | execute tool definitions | `maestro-models`, `maestro-agent`, `maestro-tui`, `maestro-theme`, `maestro-tool`, `maestro-text`, `maestro-json`, `maestro-process`, `maestro-listing`, `maestro-resources` | 2 |
| `maestro-export` | serialize session documents | `maestro-session`, `maestro-models`, `maestro-tools`, `maestro-theme`, `maestro-tui`, `maestro-json`, `maestro-text` | 3 |
| `maestro-extensions` | govern extension semantics | `maestro-models`, `maestro-agent`, `maestro-session`, `maestro-catalog`, `maestro-tools`, `maestro-theme`, `maestro-tui`, `maestro-resources`, `maestro-listeners` | 3 |
| `maestro-app` | coordinate application operations | `maestro-protocol`, `maestro-terminal`, `maestro-markdown`, `maestro-models`, `maestro-agent`, `maestro-credentials`, `maestro-settings`, `maestro-catalog`, `maestro-session`, `maestro-tools`, `maestro-resources`, `maestro-packages`, `maestro-extensions`, `maestro-export`, `maestro-theme`, `maestro-tui`, `maestro-watch`, `maestro-text`, `maestro-json`, `maestro-process`, `maestro-listing`, `maestro-daemon`, `maestro-workspace`, `maestro-listeners`, `maestro-lock` | 4 |
| `maestro-extensions-wasmtime` | execute artifacts (optional path utility) | `maestro-extensions` | 4 |
| `maestro-chat` | present interactive conversations | `maestro-app`, `maestro-tui`, `maestro-tui-crossterm`, `maestro-theme` | 5 |
| `maestro-cli` | present command-line operations | `maestro-app`, `maestro-tui`, `maestro-tui-crossterm`, `maestro-theme` | 5 |
| `maestro-rpc` | present JSONL RPC contracts | `maestro-app`, `maestro-theme` | 5 |
| `maestro-web` | present browser interactions | `maestro-app`, `maestro-theme` | 5 |
| `maestro-multiplexer` | present the workspace multiplexer | `maestro-app`, `maestro-protocol`, `maestro-tui`, `maestro-tui-crossterm`, `maestro-theme` | 5 |
| `maestro` | compose executable entry points | `maestro-theme`, `maestro-tui`, `maestro-tui-crossterm`, `maestro-process`, `maestro-app`, `maestro-cli`, `maestro-rpc`, `maestro-chat`, `maestro-web`, `maestro-extensions-wasmtime`, `maestro-multiplexer` | 6 |

`maestro-cli` is a command-line frontend, not an interactive view. Its `config_selector`
module is only the wrapper that runs a chat-owned configuration view through a
supplied presenter; every selector widget class stays chat-owned. The conventions
check accepts that one module declaration at `src/input/mod.rs` and rejects widget
declarations or the same module anywhere else in the crate.

The sealed local/thread-safe callable representation (future, shared and weak pointer types) belongs to `maestro-models`, the lowest existing callback owner. Models also owns the provider callable, payload/response hook and options family, and provider submission representation. One retained provider or scripted factory is callable in both families with a family-associated return. Agent-specific callback signatures, conversion from existing task admission and typed local entry belong to `maestro-agent`; both representations reuse its single driver and lifecycle. Local registered dispatch requires caller-supplied submission; replacement-stream dispatch requires none. Native registered objects remain thread-safe even when accepting local request hooks. No crate edge changes. See [agent execution affinity](../adr/agent-execution-affinity.md).

`maestro-tools::BashExecution::Operations` selects the retained shell operation
bounds. Local operations may retain caller-local state; threaded operations retain
`Send + Sync`. Holders use `E::Shared<E::Operations>`: `Rc` for local operations
and `Arc` for threaded operations. Both families borrow their original output
sink only during execution and share the existing native driver.

`maestro-models` serves FTP proxy script sources with its own native client inside the
existing source loader; no crate gains an FTP dependency and
`workspace-crates.json` gains no edge. That client owns the plain-FTP subset the loader
uses: connecting and greeting, logging in, binary and file-structure settings, extended
and classic passive preparation, the modification time, one directory listing and one
download. Each connection keeps its own byte-activity window, one sender frames every
command so a credential never reaches a diagnostic, and the loader keeps the
modification/cache decision. FTPS, uploads, active FTP and public client options are not
part of that caller.

`maestro-cancellation` owns cooperative cancellation below layer 0. Its only
internal dependency is `maestro-listeners`, which keeps the synchronous abort
observers of each signal. Only models and the terminal toolkit may depend on it.

`maestro-text` and `maestro-json` are core leaves below layer 0 with no workspace
dependencies, including `maestro-path`. Text owns common text operations: whitespace,
lexical comparison, markup escaping, CSS string serialization, locale construction and optional locale-number
formatting shared by transcript components. JSON owns JSON boundary
semantics: object acceptance, canonical keys, truth, selected-value display, compact and pretty text, and JSONL line framing.
`maestro-entry` and `maestro-tool` are core record contracts below layer 0 that depend
only on `maestro-request`: entry defines session interchange records, including the
`ForkPosition`, `NavigateTreeOptions` and generic `CompactionResult<T>` shared by native generation and guest authoring; tool defines
tool interchange records. `maestro-brand` is a core domain library below layer 0 that
depends on text and JSON and projects a brand pack. These five owners are declared by
policy and exist. `maestro-tool` owns `ToolExecutionMode`, `AgentToolResult`,
`ReadToolDetails`, `TruncationBy`, `TruncationResult` and `BashResult`; native and guest
public paths specialize these declarations with their existing field types. Read-input
admission remains separately owned by the native tools and guest authoring libraries.
The shared omission predicate `Omittable` belongs to `maestro-request::presence`.
No `common`, `utils`, `shared-types` or background-runtime crate is added. Terminal escape stripping and binary-output control filtering belong to `maestro-text`,
shared by tools and opt-in terminal Markdown content emission. Tools retain their existing
public display-helper paths. Markdown cleaning runs after decoding, before styling or
hyperlink generation; assistant text, thinking, and expanded branch and compaction
summaries enable it. User-authored Markdown leaves it disabled.

`maestro-tools` may depend directly on `maestro-text`, the
single owner of newline normalization shared by the edit tool, frontmatter and the editor.
Tool renderer field selection reuses `maestro-json` for JSON truth and selected-value
display. Callers keep field lookup and defaults; the leaf borrows truthy strings and
writes other truthy values with its compact JSON writer. `maestro-agent` reuses that
leaf's truth classification for the falsy reconstructed tool-call argument roots its
proxy stream falls back from; the edge carries no other coupling.

`maestro-request` defines shared message and model-request records: model
descriptors, messages, content, usage, stream events and diagnostics, plus
`SourceInfo`, `Skill`, `SlashCommandInfo` and `SlashCommandSource` for
instruction inputs and command provenance. Resources, application presentation data and
the guest re-export the command records; command discovery stays with its owner.
Shared application messages include `BashExecutionMessage`,
`CustomMessage`, `BranchSummaryMessage` and `CompactionSummaryMessage`, with the
existing `Presence<T>` optional-field wrapper, and the custom-message renderer
option record `MessageRenderOptions`, `CustomMessageInput<T>`, `MessageDelivery`,
`UserMessageDelivery`, `SendMessageOptions` and `SendUserMessageOptions`, re-exported by session and the guest. It owns the declarations and their
serialization, plus the resolved request-auth record, default zero usage and the shared
optional-field helper; `Presence<T>` stays single-source here. Its `oauth` module owns
`OAuthCredentials`, `OAuthPrompt`, `OAuthAuthInfo`, `OAuthSelectOption` and
`OAuthSelectPrompt`, re-exported by models and the guest. Its `auth_storage`
module owns `ApiKeyCredential`,
`AuthCredential`, `AuthStorageData`, `AuthSource` and `AuthStatus` with their
serialization; models and credentials re-export them, and parsing, stored-record
construction, locking, key selection and persistence stay with credentials. Its
`provider_config` module owns `ProviderConfig<C>` and `ProviderModelConfig`; the
catalog specializes `C` with its callable provider operations, the guest leaves it
empty, and validation, replay and registration stay with the catalog.
Existing models, resources and credentials public paths re-export their moved records. Provider
invocation, scheduling, clock-dependent diagnostics, resource discovery and
provenance construction stay with their existing owners; this is not a general
shared-types container.

Shared invocation options also live in `maestro-request::types`: `ThinkingBudgets`,
`CacheRetention`, `Transport`, `ProviderResponse`, `StreamOptions`,
`ProviderStreamOptions`, `SimpleStreamOptions` and `StreamOptionsInput<C, O, T>`.
The input enum preserves the original raw or simple variant, parametrizing only
common options, opaque objects and tool choice; native and guest aliases retain
their own capability types. It borrows the common options without converting
between the variants. `StreamOptionSlots` selects the signal, payload hook,
response hook and fetch representations without operations.
Native aliases specialize the fetch slot so unannotated SDK constructors still
coerce transport closures; they retain execution-family identity and defaults. Guest aliases
retain local capabilities. Provider execution, opaque native objects and tool
choice behavior stay in models.

`maestro-resources` owns the package manifest reader (the `maestro` member of `package.json`),
extension entry resolution and the project configuration directory name `.maestro`. A
directory's extension entries are the existing paths its manifest's `extensions` list
names, in list order. Otherwise they are the first of `index.ts`, `index.js` and
`index.wasm` that exists. Extension files end in `.ts`, `.js` or `.wasm`, matched
case-sensitively. Package discovery and extension path discovery call this one owner.
It uses `maestro-json` for JSON truthiness.

The read-only footer contract `ReadonlyFooterDataProvider`, its live
`ExtensionStatuses` view and the one generic insertion-ordered live map behind it
belong to the extension consumer layer (`maestro-extensions::footer_data_provider`),
because extension footer factories receive it. The application keeps the footer
metadata owner, its branch, watch, count and status policy and its existing public
paths, which re-export the moved declarations. The keyed live map reuses
`maestro-listeners::Listeners` for insertion-order traversal. Replacement keeps
position; removal returns ownership for release outside collection borrows.
Clones retain one collection, and active walks observe edits until exhaustion.

The replaceable editor contract `EditorComponent` belongs to the toolkit with its
optional operations and their fallible forms, which delegate to the plain operations
unless a component-backed editor reports its own failure. The application capability
of a replacement editor (`CustomEditorComponent`), the editor factory result
(`EditorFactoryOutcome`) and the shared action map (`ActionHandlers`) belong to
`maestro-extensions::types::ui`, because extension editor factories return them. The
application re-exports them; the chat frontend's `CustomEditor` implements the
capability and keeps the action map alias at its existing path. Extension components
carry their optional disposal, expansion and overlay width in the same owner.
Native repository discovery belongs to `maestro-workspace::workspace::git`.
The daemon and application footer reuse its directory walk, lexical path and drive
resolution and shared Unicode trimming without replacing native path bytes.
A typed discovery policy preserves caller-specific metadata rules: the
footer requires the spaced worktree marker, accepts an existing HEAD of any kind,
and reads `commondir` only for worktree links; it stops at a marker without HEAD.
Workspace identity climbs past markers without a regular HEAD, accepts the
unspaced marker, requires a regular HEAD file and reads shared metadata for all
repository layouts, falling back to the checkout metadata if `commondir` cannot
be read. Both policies support standalone bare repositories. The footer
keeps branch, detached-head and watch policy; Git subprocess
fallbacks stay with the process-owning callers. The application reaches discovery
through a native-only dependency on the workspace owner, not a daemon re-export. Use native files
rather than a new injected discovery port whose only other adapter is a test fake.


The crate is core, sits below layer 0 and depends on no workspace crate, including
`maestro-path`. Only the table entries that list it
(models, resources, theme, catalog, session, entry, tool and the guest) may depend on it. The guest still cannot
depend on models or resources, the runtime adapter still targets extensions with the optional native path utility, and no internal dev edge is added. Existing delivery
layers stay unchanged; the shared owner precedes its consumers.

`maestro-pattern` is a core crate below layer 0 whose one job is matching file
patterns: glob patterns and ignore rules. Its only workspace dependency is
`maestro-text`; it never depends on `maestro-path`. Only `maestro-resources`, `maestro-packages` and `maestro-catalog` (for model-scope
matching) may depend on it. Catalog owns model-scope selection and reuses the glob matcher.
Move the existing matchers only after this contract lands, through
[shared file-pattern ownership](https://github.com/Orchestration-Maestro/maestro/issues/561).

`maestro-watch` is a core leaf below layer 0 without workspace dependencies.
`maestro-lock` is below layer 0 and depends only on `maestro-path`, which owns
`native_homedir`; it uses the existing external `sha2 =0.11.0` for target identity. Only theme and application
may depend on watch; only settings, credentials, workspace, detection, session and application
may depend on lock. Application uses maestro-lock for recoverable file publication.

`maestro-watch::fs_watch` owns watch creation/close helpers, the injected
watch/timer interface and the native notification adapter. Reload, refresh,
retry and polling policy remain in their feature owners. Existing theme watch
paths may re-export the same moved types, not retain a second implementation.

`maestro-lock` owns bounded and waiting acquisition of caller-opened file-write
locks. Waiting acquisition uses the operating system's file lock, which is released
when the holder exits. It also owns recoverable in-place byte publication, canonical target identity
and the per-user lock root described in [file locking](../locking.md).
A durable copy of the intended bytes is replayed by the next holder after an
interrupted write. Native ownership, access controls and hard links survive.
Workspace snapshot storage, detection catalogs and session rewrites use this
operation; feature owners retain serialization and symbolic-link policy.
Settings and credentials retain their existing bounded, in-place persistence.
Neither leaf introduces a common background runtime, scheduler,
retry policy or listener framework. Feature-specific timing, process,
subscription and teardown semantics and current public capabilities are retained.

`maestro-listing` is a core leaf below layer 0 with no workspace dependencies,
including the path utility. Its one job is listing directory entries: it returns
one directory's entries sorted by file name on every platform. File systems return
entries in their own order, such as hash order on many Unix file systems and
case-insensitive name order on NTFS. The JavaScript runtime sorts its listing on
Unix only; that platform difference is a runtime mechanic, so the leaf does not
copy it. Only resources, theme, the terminal toolkit, tools, session and the
application may depend on it, and only on native targets. Callers keep their own kind filtering,
name decoding and any further ordering. Guest-authoring examples may also use the leaf
through one portable development dependency from `maestro-extensions-wasm`; this
permits no normal or build edge.

`maestro-listeners` is a core leaf below layer 0 with no workspace dependencies.
It owns one job: retaining callbacks in live registration order. A callback is
admitted once per identity, so registering a live one again keeps its first
position, while removing and re-registering it moves it to the last position.
A caller may instead admit every registration as its own entry, as an
event-emitter listener list keeps them; each removal operation then removes its
own registration. Traversal sees the collection as it is at each step. Models,
the terminal toolkit, the agent, the application, the extension runtime, the
terminal adapter and the cancellation leaf may depend on it.
Each caller keeps its own borrowing, locking, delivery and settlement policy:
the leaf holds no lock, runs no callback under its own borrow and returns removed
ownership to the caller. Its storage is a flat ordered list plus an identity
index; retired slots are reclaimed while no traversal runs, so one removal costs
expected constant work and clearing every registration costs work proportional to
their number. Traversal bookkeeping is per walk and never retains callbacks.
Callers that need emission-time membership use an owned snapshot of the registered
handles.

Implement these owners only after this amendment lands, through
[filesystem-watch ownership](https://github.com/Orchestration-Maestro/maestro/issues/493)
and [file-lock acquisition ownership](https://github.com/Orchestration-Maestro/maestro/issues/494).
Size each relocation by net added production lines, showing matching moved and
deleted lines; moving lines is not a code saving.

`maestro-process` is a core leaf below layer 0 with no workspace dependencies,
including the path and cancellation leaves. It owns native child-process lifecycles:
captured-child creation, raw pipe transfer and collection, child-exit and
pipe-completion observation, termination (including a caller-selected post-exit grace and TERM-then-KILL escalation) and reaping. Only tools, packages, credentials, detection, the application and the
terminal toolkit, terminal adapter and the daemon may depend on it, and only on native targets; callers keep their
cancellation, termination and settlement policies. Shell discovery, environment
construction, command prefixes, tool definitions, output decoding, sanitization and
truncation, artifact files, presentation, result mapping and user-facing errors stay
with their feature owners. The process leaf also owns the retained-front byte queue
shared by pane-process input and native terminal output: ordered admission, partial
write advancement and failure cleanup. Callers choose the write chunk bound and
keep descriptor readiness, lifecycle and error-reporting policy. It owns native
termination-signal subscription and idempotent removal; a native mode supplies the
callback and keeps its exit policy. No frontend subscribes to native signals
itself. It also owns the current process's native signal effects: the default action a
prepared termination signal or interrupt takes, the job-control stop of the process
group and the observation of its continuation. Callers keep when to prepare, restore,
stop and resume.
The shared launch contract includes synchronous capture
with optional raw stdin, an optional caller-supplied timeout and an optional
combined captured-byte bound, inherited standard
descriptors without a timeout, and background input transfer and direct-child
reaping after acknowledged native startup. For capture and background launches,
absent input uses the null device; present input closes its pipe after writing.
None or zero disables the timeout; a selected timeout covers input transfer,
child exit and captured pipe EOFs. None or zero also disables the byte bound;
overflow stops collection, closes unfinished stdin and attempts to force-kill/reap
the direct child through the same waiter, returning a wait failure.
On expiry, it attempts to force-kill and reap the direct child if it is still running;
native errors can prevent this. If the direct child has already exited, it closes
unfinished pipe operations without signalling that child. It adds no general background
runtime or default deadline. The
conventions test checks dependency ownership across declaration forms, not target
predicates; native-only manifests and the existing wasm and browser builds are
qualified when the crate is extracted through
[native process ownership](https://github.com/Orchestration-Maestro/maestro/issues/695).
This declaration creates no crate and moves no code.

`maestro-path` is the foundation utility for lexical path strings. It sits below
layer 0, has no internal dependencies, and exposes platform-independent path
operations. Native path normalization shares its segment reducer with the string
flavors and preserves native component bytes without filesystem observations.
Native crates may depend on it without changing their delivery layer.
This optional utility edge is ignored when evaluating leaf status and exact
direct-dependency sets; every other dependency rule, including cycle and internal
dev-dependency checks, still applies. The guest authoring crate
(`maestro-extensions-wasm`), terminal scenario harness
(`maestro-test-terminal`), checked test-process owner (`maestro-test-process`), disposable directory owner
(`maestro-test-directory`), fixture owner (`maestro-test-fixtures`), test tracing owner (`maestro-test-tracing`), cancellation leaf (`maestro-cancellation`), shared
request owner (`maestro-request`), watch leaf (`maestro-watch`), process leaf (`maestro-process`), listing leaf (`maestro-listing`), file-pattern owner (`maestro-pattern`),
Markdown parser (`maestro-markdown`), text leaf (`maestro-text`),
JSON leaf (`maestro-json`), entry and tool record contracts (`maestro-entry`,
`maestro-tool`) and brand projection (`maestro-brand`) are excluded from this
permission, so being a text consumer does not authorize the utility. Shared path helpers
take supplied directory observations, except `native_homedir` reads the native
home environment and account entry. They add no filesystem service,
directory cache, scheduler or workspace dependency.
Final-component link traversal uses a caller-supplied observation callback:
the utility owns relative-target joining and the forty-link bound, while lock
and the native component adapter keep native link I/O and its error interpretation.
Both may declare the native utility edge; it adds no cycle because the utility
has no internal dependencies.

### Native feature unifier

`maestro-dependencies` is the build-only feature unifier, generated and verified
by `cargo hakari`. It is a dedicated, unpublished crate with an empty library, no
public API and no workspace dependencies, including the path utility. Every other
workspace member may declare it only as a non-optional normal dependency under
`cfg(not(target_arch = "wasm32"))`, so browser and component builds never reach it.
It never satisfies a required or exact direct dependency and never changes a
member's layer. The crate is the sole exception to member `serde_json` inheritance;
the workspace precision declaration and lint inheritance still apply to it.
Never hand-edit its generated manifest section.

The terminal adapter (`maestro-tui-crossterm`) requires the toolkit and
`maestro-text` as direct dependencies; an absent required dependency fails, and the
terminal scenario harness remains forbidden from importing text.

The internal development edges include the test-process edges below. `maestro-tui-crossterm` to
`maestro-brand` serves the terminal examples' shared brand projection. It does not
permit a normal or build dependency on brand or theme, satisfy either required
toolkit/text dependency, or make the scenario harness a development target.
`maestro-extensions` to `maestro-credentials`, declared for native targets only,
lets extension tests and doctests build a real model catalog over in-memory
credential storage; it permits no normal or build dependency on credentials.
The native component adapter (`maestro-extensions-wasmtime`) may depend on
`maestro-catalog`, `maestro-credentials`, `maestro-models`, `maestro-session`,
`maestro-theme` and `maestro-tui` as native-only development dependencies. Its composition tests
reuse the extension test support to build real catalogs, credentials, messages,
sessions and themes. The toolkit edge supplies the real terminal and UI to observe
component-factory width synchronously; it adds no new production layer crossing.
These edges permit no normal or build dependencies.
`maestro-detection` to `maestro-text`, declared as a non-optional native development
dependency, supplies terminal newline normalization for checked-child tests.
`maestro-rpc` to `maestro-theme` and `maestro-tui-crossterm`, declared as native
development dependencies, let the standard-stream child tests build the real export
rendering environment; the adapter's normal dependency stays `maestro-app` alone.
`maestro-chat` to `maestro-models`, declared as a non-optional native development
dependency, lets interactive-mode tests drive a real session through the scripted
provider; it permits no normal or build dependency on models.
`maestro-cli` to `maestro-listeners`, declared as a non-optional native development
dependency, supplies retained registration snapshots for the controlled print adapter.
It permits no normal or build dependency on the listener leaf.
`maestro-chat` to `maestro-tui-crossterm`, declared for native targets only and
without optionality, lets the interactive mode's composed witness drive a real
native terminal; it permits no normal or build dependency on the adapter and does
not make the interactive frontend a terminal owner.
`maestro-extensions-wasm` to `maestro-models`, declared as a native-only,
non-optional development dependency, lets component tests pass retained schema
callbacks to the native argument validator. It permits no normal or build edge.
`maestro-extensions-wasm` to `maestro-listing`, declared as a portable development
dependency, supplies name-ordered directory reads to the guest examples. It permits
no normal or build dependency on the listing leaf.
These edges remain subject to test-graph cycle checks, including aliases and
platform-specific declarations. Add the example dependency and replace its local
projection only after this contract lands.

### Terminal child setup entry

`maestro-process` owns the parent-side terminal launch and the child-side setup
entry. The composition root calls `maestro_process::pty::setup_entry` before
ordinary argument parsing. A setup invocation acquires its terminal session,
resets signals and replaces itself with the requested command. The parent passes
an explicit executable and entry arguments; libraries never assume the current
executable implements the entry. Re-execution avoids an unsafe post-fork hook.

Daemon runtime tests use the checked test-process owner to select a child test
that calls this same entry. The test-process leaf does not depend on process.

### Test process owner

`maestro-test-process` is a dedicated, unpublished, native test-support leaf
that starts one named libtest child and proves the child started exactly that
test. On Unix it runs caller-supplied setup text in the interpreter the caller
names by absolute path, so it carries no shell lookup, and it uses no external
crate. It has no workspace dependencies except the native feature unifier and
one native development edge to the disposable directory owner for its own child
fixtures, and it is excluded from the foundation utility permission. Its only
incoming edges are
non-optional development dependencies under `cfg(not(target_arch = "wasm32"))`
from `maestro`, `maestro-agent`, `maestro-app`, `maestro-catalog`, `maestro-chat`,
`maestro-cli`, `maestro-credentials`, `maestro-daemon`, `maestro-detection`, `maestro-extensions`, `maestro-extensions-wasmtime`, `maestro-ipc`, `maestro-lock`, `maestro-models`, `maestro-packages`, `maestro-process`,
`maestro-resources`, `maestro-rpc`, `maestro-session`, `maestro-settings`,
`maestro-test-conventions`, `maestro-theme`, `maestro-tooling`, `maestro-tools`, `maestro-tui`
and `maestro-tui-crossterm`; a
caller declares it only when it uses it. No normal, build, optional, unqualified or
non-native declaration, no other consumer and no other `maestro-test-*` crate is
admitted by this rule. The terminal scenario owner below states its own single
declared caller. The edge is checked in declared metadata (native placement) and in
host-resolved metadata, before the dependency-free and core-to-dedicated rules, and
remains subject to both cycle checks. A live pipe-closure transaction first uses
`run_alone` to run in a fresh single-test process, avoiding sibling forks that
could inherit its pipe ends. File-backed proof accepts only an empty file with
its cursor at zero, so earlier bytes cannot prove that the selected test started.
`TestProcess::invoke_with` checks eligibility and supplies exact harness arguments
to a caller-owned invoker without claiming startup. Such callers observe the
one-test prologue through their own transport, followed by product readiness and
completion. `output_with` uses this same selection operation and additionally
checks its returned output's prologue. `TestProcess::temp_dir(path)` sets the
variables the child's temporary directory comes from: `TMPDIR` on Unix, `TMP` and
`TEMP` on Windows. A cleared child of `run_per_environment` keeps the parent's
temporary directory unless its case sets one. `user_cache_home()` names the directory a
child passes as `XDG_CACHE_HOME` to keep this process's user cache when it replaces
`HOME` on Linux, the only platform whose cache location follows that variable. `local_zone_follows_tz()` states the
premise of a test that selects a time zone through `TZ`: Unix hosts follow it, while
Windows reads its system zone, so such a test reports the premise and returns there.

### Test directory owner

`maestro-test-directory` is a dedicated, unpublished, native, std-only leaf for
exclusive disposable-directory allocation and cleanup. It has no workspace
edges except the native feature unifier and is excluded from the foundation
utility permission. Its only incoming edges are non-optional development
dependencies under `cfg(not(target_arch = "wasm32"))` from `maestro`, `maestro-app`,
`maestro-catalog`, `maestro-chat`, `maestro-cli`, `maestro-credentials`,
`maestro-daemon`, `maestro-detection`, `maestro-export`, `maestro-extensions`,
`maestro-extensions-wasm`, `maestro-extensions-wasmtime`,
`maestro-ipc`, `maestro-listing`, `maestro-lock`, `maestro-models`,
`maestro-packages`, `maestro-process`, `maestro-resources`, `maestro-rpc`,
`maestro-session`, `maestro-settings`, `maestro-terminal`, `maestro-test-conventions`,
`maestro-test-process`, `maestro-theme`, `maestro-tooling`, `maestro-tools`,
`maestro-tui`, `maestro-tui-crossterm`, `maestro-watch` and
`maestro-workspace`; a caller
declares it only when it uses it. Aliases follow the same rules. Normal, build,
optional, unqualified, browser and foreign-target declarations are rejected.
Both dependency graphs remain acyclic; no crate-name pattern grants access.

`Scratch::new()` chooses the native temporary parent and retains the created
root's canonical path; on Windows that path drops the verbatim `\\?\` prefix
of a drive path, so `/` keeps separating components. `temporary_root()` reports
that parent for tests that observe it without owning it.
`Scratch::new_in(parent)` preserves native path-join spelling, including relative
and non-Unicode parents. Both return `io::Result`,
use PID/atomic-sequence candidates and exclusive `create_dir`, retry only
`AlreadyExists`, and never delete a colliding entry. The borrowed `path()` stays
owned by `Scratch`; drop attempts recursive removal without panicking, including
when canonicalization fails after creation. On Unix, `enter_removed(name)` makes
a new directory in the tree the process working directory and removes it, so
reading the working directory fails with `NotFound`. `enter_unreadable(name)`
enters a directory deeper than the kernel path limit below `name` and takes read
permission from `name`, so reading the working directory fails with
`PermissionDenied` while relative paths still work; it returns the guard that
restores `name`, or `None` when the process still reads it. Both change
process-wide state, so only a process that runs one test calls them.
`deny(path, mode)` sets a Unix mode and checks that the process is now refused
what the mode takes from the owner. It returns the guard that restores the earlier
permissions, or `None`, with them already restored, when the process is not
refused, as in a privileged run; callers then skip the denial. `keeps_distinct(names)`
creates a set of leaf names in one directory below the owner and reports whether the
host stored that many entries. `folded_names(paths)` compares every component of each
supplied relative path with the other names its own directory holds and returns the
groups the host does not keep apart. Both let a caller state a premise a folding or
restrictive host cannot exercise. The leaf owns no caller policy, process launcher,
timer, retry limit or ownership-transfer API.

### Test fixture owner

`maestro-test-fixtures` is a dedicated, unpublished, portable leaf for shared
fixture records, recorded-header lookup, wire-literal round trips and single-color
theme construction. It has no product dependencies or foundation utility edge;
its only internal normal dependency is the native feature unifier.
Fixture development callers are `maestro-agent`, `maestro-models`, `maestro-request`,
`maestro-catalog`, `maestro-chat`, `maestro-credentials`, `maestro-tools` and
`maestro-export`; declarations may be portable or target-specific, including aliases.
Models, request, catalog, chat and credentials use portable development declarations;
agent, tools and export use native development declarations. No normal or build
edge to this owner is permitted. Both production and test cycle checks apply.
The policy and inventory admit this owner before its first member exists.

### Test tracing owner

`maestro-test-tracing` is a dedicated, unpublished, native leaf that runs a
test action under a scoped tracing subscriber that sees every callsite. It has
no workspace edges except the native feature unifier and is excluded from the
foundation utility permission. Its only incoming edges are non-optional
development dependencies under `cfg(not(target_arch = "wasm32"))` from
`maestro-daemon`, `maestro-models`, `maestro-process`, `maestro-terminal` and
`maestro-workspace`; a caller
declares it only when it uses it. The directory owner's declaration, alias and
cycle rules apply.

`with_default(subscriber, action)` keeps one subscriber that records nothing
registered for the life of the test process, then runs the action under
`tracing::subscriber::with_default`. `with_subscriber(future, subscriber)`
keeps the same subscriber registered and makes `subscriber` the default
whenever the future is polled. Dropping the future releases its dispatcher
handle; the subscriber lives until its last retained handle drops.
Callsite interest is cached process-wide and rebuilt when dispatchers register
or the interest cache is explicitly rebuilt. While only one subscriber is
registered, `tracing` decides a callsite's
interest from the registering thread's own subscriber. Without the kept
subscriber, a sibling test thread with no subscriber can cache disabled interest
for the capturing thread too, until a rebuild. The conventions check therefore
rejects `tracing::subscriber::with_default` and `set_default`,
`tracing::dispatcher::with_default` and `set_default`, and the `with_subscriber`
method in test-owned sources outside this leaf.

`Output` is the byte sink a formatting subscriber writes to. Every clone
appends to the same capture; `text` reads it and `clear` empties it.

The graph consists of the table entries, the foundation utility, the native
feature unifier and these development-only exceptions. The
[architecture conventions test](../../crates/maestro-test-conventions/tests/architecture.rs)
checks the table against the enforced dependency policy, including permitted
utility edges and leaves. The below-layer-0 owners are delivered before their
consumers; the utility is delivered before its first consumer.

### Terminal scenario owner

`maestro-test-terminal` is a dedicated, unpublished, native crate that drives the
toolkit through its public interface. Its library owns the recording terminal and
the manual runtime a controlled terminal scenario needs; its integration tests own
the virtual terminal and the written scenarios. It requires `maestro-tui` and
nothing else from the workspace, and it is excluded from the foundation utility
permission. Its only incoming edge is a non-optional native development
dependency from
`maestro-cli`, which presents the configuration and session selectors under
controlled effects; a caller
declares it only when it uses it. The directory owner's declaration, alias and
cycle rules apply, and the required-toolkit rule keeps `maestro-text` out of it.

### Workspace daemon

The workspace daemon is core design. The executable requires a direct application
dependency, with daemon operations reached through `maestro-app`. A direct
multiplexer dependency is permitted when its launcher is wired, not required for
command-line startup. There is no Cargo feature, optional dependency or separate install.
Defaults belong to the daemon. The daemon and multiplexer are native-only; browser
builds already exclude native process crates. Every frontend calls `maestro-app`.

The workspace daemon, the multiplexer and their commands share the application's single setup. They
keep no configuration file, plugin registry, state or log root, keybinding file, installer or update
path of their own. Their preferences are members of the layered `settings.json` under `multiplexer`,
`daemon` and `update`. The daemon reads and writes only the global scope. Multiplexer bindings and
custom command bindings are members of `keybindings.json`. Action manifests are package content of
the `actions` resource kind, installed, updated, listed and removed by the package manager. Daemon
files, sockets and logs live under the configuration root through one path table. Release checks, the
detection catalog, remote installs and announcements use one hosted fetch, which the offline switch
stops. An input format that users already hold may be read as an input alias of the one owner. It is
never written back and never becomes a second setting, registry or format.

Maestro has one protocol: the 29 established session commands and all workspace commands share one
command set, the established `type`-tagged request and `response` envelopes, one `type`-tagged event
format and one protocol version on both stdio and daemon sockets. The 29 session commands keep their
exact wire bytes, fields, omission/null rules and results. Workspace commands are additive on stdio.
`maestro-protocol` owns the generic request and response envelopes, the workspace records and
their schema; `maestro-app::rpc_types` owns the session records and the composed command
family; `maestro-json::jsonl` owns JSONL framing. `maestro-app::rpc_mode` is the one connection
runner: it serves workspace operations and an optional local session over a caller's line
reader and writer, returning how the connection ended. `RpcModeOptions` groups session-only
facilities in `Option<RpcSessionOptions>`: runtime, process signals, theme and export effects.
A session-free connection creates no UI or session binding and registers no session signals;
session commands return a correlated refusal asking the caller to open in a session. A supplied
session keeps its binding, rebind, event subscription and disposal lifecycle.
`maestro-rpc` is its standard-stream adapter, which exits the process. Live screen traffic stays binary in
a channel of the same protocol and version. Plain JSONL clients need no new header, handshake or
version member, and receive no unsolicited startup record or binary bytes.

`maestro-terminal` is a pure-Rust terminal emulator, specified in
[terminal emulator](terminal-emulator.md): no foreign code, no build script and no
unsafe code. The existing guest-authoring generated-binding lint exception is
unchanged.

### Crate order

A crate is reached when every crate it depends on has its first tickets landed. Each crate's tickets are written when that crate is reached, and each ticket links to this specification. Tickets are not planned in advance for all crates. Same-layer crates run in parallel (at most 4 compiling lanes).

**Below layer 0:** `maestro-protocol`, `maestro-ipc` join the existing foundation leaves.

1. **Layer 0:** `maestro-extensions-wasm`, `maestro-models`, `maestro-resources`, `maestro-settings`, `maestro-test-conventions`, `maestro-test-directory`, `maestro-test-fixtures`, `maestro-test-process`, `maestro-test-tracing`, `maestro-tooling`, `maestro-tui`, `maestro-terminal`, `maestro-workspace`, `maestro-detection`
2. **Layer 1:** `maestro-agent`, `maestro-credentials`, `maestro-packages`, `maestro-test-terminal`, `maestro-theme`, `maestro-tui-crossterm`, `maestro-daemon`
3. **Layer 2:** `maestro-catalog`, `maestro-session`, `maestro-tools`
4. **Layer 3:** `maestro-export`, `maestro-extensions`
5. **Layer 4:** `maestro-app`, `maestro-extensions-wasmtime`
6. **Layer 5:** `maestro-chat`, `maestro-cli`, `maestro-rpc`, `maestro-web`, `maestro-multiplexer`
7. **Layer 6:** `maestro`

## Completion and explicit later scope

Owner-approved tool and terminal additions are specified in [tool and terminal additions](tool-additions.md).

Completion requires the entire file/function/test/fixture matrix, all R01–R12 acceptance, full shipped examples/docs, platform prerequisites and selected service destinations to close with no unapproved behavioral omission. A generic abstraction, a passing subset or a complete file map does not substitute for the delivered capability. The library equivalents are supervisor-selected and qualified on the reference inputs under the common semantic and boundary contract. A mismatch requires matching results through the owning Rust implementation, not an engine port or caller workaround. Added task scope still goes through plan approval.

Deferred model responses, remote core catalog refresh, newer transcript/tool-change replay, nested tool calls, cache warming, project trust and the four later RPC commands are not part of this baseline. Classification, generated images, embeddings/reranking, `maestro init`, native out-of-process extensions, alternate default session storage, jobs and added MCP/supervisor capabilities remain later owner phases. Their absence does not justify omitting baseline examples, browser discovery, JSONL persistence or existing conversions.

Manifest mode and its settings locks are specified in [manifest mode](manifest-mode.md).

**Reference defects.** Correct known and encountered defects in their owning mapped files; list each correction once with a regression. Correct stale documentation to the delivered behavior. Do not mark an affected file complete until its fixes pass; an example or adapter without a mapped owner is a coverage gap, not completed delivery.

**Replaceable visual identity.** One design-token data file supplies Maestro colors, font and mark metadata to browser pages, the exported viewer and shipped terminal dark/light defaults. Embed whole Barlow, Barlow Condensed, JetBrains Mono and Space Grotesk fonts with OFL notices for browser/viewer text; Noto Sans Symbols 2 supplies missing symbols without replacing a text role. Use the Maestro M mark. A same-format user override replaces the identity without code changes. Preserve layout, behavior, terminal theme format/names/keys and custom themes; the terminal font remains user-controlled.

**CI placement.** Shared CI is the whole merge check. The caller's `.github/ci.toml` lists `wasm_crates`, four platform `check_targets` and optional `browser_build`; absent configuration skips only optional jobs, invalid configuration fails. As #109 specifies, `just test` remains unwrapped; the separate ported non-LLM test script removes the specified provider credentials and sets the agent's auth file aside, restoring it on exit. Required shared jobs use real rg/fd, wasm builds and foreign-target checks. Enabling the browser application runs the caller's mise-pinned `just browser-build` recipe for real app/example assets. Release binaries, notes, checksums, SBOM/attestation and smoke belong to this repository's tag workflow. macOS/Windows runtime tests return before first release. No separate gate program or extra required repository workflow is added.

**Approved scope exclusions.** Hidden joke commands and their views are not part of Maestro. Omit their dedicated assets and model-selection checks. Rust file paths use the current source-keyed name map; source-based ownership does not change.

`maestro-markdown` owns native Markdown structure, owned source extents and mapped
extended links. The toolkit keeps terminal normalization and formatting. The
application re-exports the parser as `markdown`; browser consumers use that edge.
Document, list-item and quote-chunk parsing share one block accumulator.
Quotes suspend and resume through retained source handles. List items enter
non-top-level grammar; completed block calls return to top level. Quote chunks save and restore the enclosing grammar while appending
to the same output. Recognized raw consumption is separate from semantic text
and its mapped source, including empty quote bodies and invisible definitions.

`Node.authored` and the body in `Kind::Quote(Vec<Node>, MappedText)` use the
parser-owned `MappedText` type. Its private storage shares owned UTF-8 source
and mapped selections; cloned values outlive their document. `Document.source`
remains a `String`, and native ranges and inline interfaces stay unchanged.
`MappedText::len() -> usize` returns the logical byte length in constant time;
`is_empty() -> bool` tests that length. `chunks() -> impl Iterator<Item = &str>`
yields the exact logical bytes in order without flattening; chunk boundaries
are not a format. `to_owned_string() -> String` explicitly materializes them.
`From<String>`, `Clone`, `Debug` and empty `Default` are supported. There is no
implicit contiguous accessor, `Deref<str>` or per-node flattened cache.

Reply-quote work and retained storage grow linearly in input bytes, admitted
nodes and mapping records, excluding requested output bytes. A list item keeps
its admission state and block output across quote resumption over the same
shared source handles. Admission resumes at the first open block: one whose
grammar read the item's current end, or whose following content sits on the
item's unterminated last line. Open indented code continues from the line it
reached. A caller block tokenizer reads the whole remaining item, so when one
is present every resumption admits the item again. Caller block tokenizers
receive the same contiguous operands in the same order, including repeated
admissions. Materializing those operands is caller-requested output work,
charged separately from parser work; contiguous source operands are borrowed.
Quote resumption transfers retained source handles rather than flattening the
remaining tail.

The parser depends internally only on text and introduces no UI or effect owner.
The dedicated conventions checker may consume its code-block source ranges for
documentation checks; it does not own a separate Markdown grammar.

JSON Pointer token escaping belongs to `maestro-json::escape_pointer_token`;
brand diagnostics and guest JSON member views reuse it.

Tool parameter schemas have one owner, `maestro-request::parameter_schema::ParameterSchema`.
`ParameterSchema<C = ()>` keeps executable annotations in its generic payload;
the default payload stays data-only. The shared `Base`, `Refinement` and
`SchemaNode` records own conversion, guard and ordered refinement callbacks.
The request-owned `SchemaCallback` retains shared captures and compares callback
identity. A defaulted callable type keeps the native signature; the guest adapter
delegates capture ownership while supplying its borrowed operand signature.
Native callbacks receive a mutable JSON value. The guest
specializes those records with borrowed JSON views and its existing callback
resource; it does not declare another schema record or callback registry.
`Tool<C = ()>` retains that payload, while ordinary request contexts keep the
data-only default. Explicit data projection strips executable annotations without
running them. Serialization never writes code, and deserialization never rebuilds it.
The record keeps the supplied JSON Schema value and, beside it, the kinds its author
declared for individual nodes, keyed by JSON pointer; a node without an entry is plain
and is never converted as an authored node. `SchemaKind` names the extensible kinds the
schema builders produce. `Tool`, `Context` and `ToolInfo` default their schema operand to
this owner, guest authoring re-exports it, and the model argument-validation owner is its
only converter. Inside that owner the supplied schema is indexed once: a data-only
document holds each authored node's kind against the node's identity, a small context owns
only the named definitions visible on the current conversion call path, and each stage and
kind keeps its own module (conversion, instantiation, evaluation, comparison, object
property modifiers). Structural comparison and evaluation receive only the operands they
act on, never the conversion context or a projection shared between kinds; conversion and
instantiation also receive the conversion context. Serialization writes the supplied JSON
alone: providers, persisted JSON
and the SDK observe ordinary JSON Schema, and nothing infers author intent from a schema
text. The guest carries the declared kinds only inside its opaque internal registration
and enumeration transport. Executable nodes use invocation-local JSON resources
and independently retained identities in the existing callback table. Same-origin
guest enumeration keeps the original callbacks and captures, with bindings per
tool. Foreign guest and native schemas retain live callbacks through the receiving
guest's existing native capability registry. Synchronous invocation uses the existing
native import route and scoped operand wait, preserving edits and structured schema
errors without copying the operand. Nested calls into the receiving instance run
locally through that route, including A→B→A. Final release uses the existing pumps;
an unloaded origin rejects later entry while already-admitted calls finish.
An absent child view passed to a foreign callback returns a clear error rather than
substituting null. See [foreign schema calls](../extensions/guest.md#foreign-schema-calls)
for the owning path and pinned runtime contracts. Ordinary result and string APIs
are unchanged.

Tool metadata has one generic record owner, `maestro-request::tool_info::ToolInfo<P>`.
Native callers use its default parameter-schema owner through models; guest authoring
keeps a `ToolInfo` alias over the same owner. Live callback storage
belongs to `maestro-listeners::Listeners<P>`, with one admission per identity by
default or one per registration as an event-emitter listener list keeps them.
Agent subscriptions, session listeners, extension error delivery, terminal input
listeners and native output-failure handlers reuse it. For shared `Rc` callbacks it
also owns subscribing, which returns the removal
operation, and the live walk in registration order. What each listener receives
and when it runs remain with the caller. The collection returns removed ownership
for release outside guards, and it reclaims retired registrations while no
traversal runs, so one removal and a complete clear both stay cheap.

Extension runner sessions and context catalog getters retain one direct shared
`ModelRegistry` holder; its internal synchronization stays catalog-owned.
The complete action runtime composes the existing activity/flag state.
Binding installs action and base-context operations before provider flushing;
command-context binding remains independent.


### Retained tool registration and shared events

Tool definitions separate callback ownership from invocation. SDK defaults remain
Threaded/Threaded; extension invocation is Local, with ownership following the
facade. Threaded definitions retain thread-safe execution, preparation and renderer
captures; execution futures follow invocation. Local definitions retain local
captures. One ordered owner keeps definitions, provenance and renderers. One
tools-owned wrapper accepts owned or retained definitions, reads retained execution
callbacks at call time and creates a fresh runner context per execution.

`maestro-request::extension_events` owns extension event/result schemas with generic
native/guest operands; `maestro-request::system_prompt` owns assembled prompt input
records and `CONTEXT_FILE_NAMES`, the context file names that resource discovery and
the read tool share. Models re-exports both modules. Native aliases select existing model,
session and tool records; guest aliases keep their existing capability carriers.
Request depends only on the dependency-free JSON leaf: the object reader stays
JSON-owned and the Presence adaptation belongs beside the shared records. Fork
position stays entry-owned. One event inventory supplies author unions and the
guest data adapter. Resources, cancellation, finite-number checks, stream snapshots
and encoding stay adapter-owned.

The parameter-schema owner also owns the component metadata transport in
`maestro-request::parameter_schema::transport`, re-exported through models and native extensions
as `schema_transport`. Guest registration and enumeration use that same codec;
ordinary provider and persisted JSON omit the carried schema kinds. This adds no
crate dependency: both extension adapters already reach the request owner through
their declared edges.

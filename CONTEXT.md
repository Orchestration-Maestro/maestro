# Glossary

The implementation is being rebuilt crate by crate; these terms describe the
foundation contract, not currently delivered capabilities.

Add terms here when the specs establish their meaning; use one name per concept.

## Models

From `docs/specs/maestro-port.md`.

- **Provider:** A named source of model descriptors and authentication behavior; a protocol adapter owns invocation and wire conversion.
- **Model reference:** A bare model ID or provider/ID accepted by [model selection](docs/selection.md).
- **Model pattern:** Authored ID/name selection text with an optional thinking suffix; [model selection](docs/selection.md) owns matching and diagnostics.
- **Model identity:** The combination of provider ID and model ID.
- **Replay identity:** Provider, API and model ID used to decide whether signed response content is reusable.
- **Socket effect:** A replaceable model-library operation opening an owned connection for complete text messages; native upgrades are shared, while caller authentication and lifetime policy stay with their operation.
- **Protocol adapter:** An implementation of a registered model protocol, including provider-specific wire conversion and usage interpretation.
- **ModelRegistry:** The owner of the usable local model catalog. [Local model loading](docs/catalog.md) provides explicit-file and memory construction, overrides, shared observations, reload, request authentication, provider metadata and dynamic registration, replacement, replay and removal.
- **Tool arguments:** The model-supplied argument value of any JSON root kind, completed from accumulated JSON before execution.
- **Tool declaration:** A model-facing name, description and JSON Schema argument contract, distinct from an executable tool.
- **Sandbox host:** The supplied platform owner of hidden JavaScript execution; the application keeps output and download formatting. See [browser tools](docs/application/browser-tools.md#javascript-execution).
- **Sandbox runtime provider:** A retained source of owned globals, complete injectable function source, helper descriptions and optional lifecycle/message hooks.
- **Sandbox value:** A caller-owned return value with fallible REPL display, without a mandatory JSON transport.
- **Model-request projection:** The selected-model view of supplied conversation context, not the stored branch or a history write.
- **Catalog override:** Partial nested metadata supplied by local model files over base catalog entries.
- **ApiProvider:** Ordered protocol registration holding raw and simple invocation callbacks.
- **EventStream:** Producer-owned FIFO with independently observable result.
- **AssistantMessageEventStream:** EventStream specialization retaining shared assistant-message handles.
- **Request:** An invocation of a supplied model descriptor with its inputs, options, authentication and cancellation; catalog membership is not required.
- **Request records:** Shared message and model-request data describing application messages and a model invocation's descriptor, conversation, instruction inputs and observations; the shared owner is `maestro-request`, not an invocation executor or a general data container.
- **Stream option slots:** The shared request owner selects signal, payload hook, response hook and fetch representations without executable operations. Shared common, raw and simple options retain their native and guest aliases. Its `StreamOptionsInput` record preserves the supplied raw or simple variant without converting their policies; provider execution stays in models.
- **Request authentication:** Invocation data authorizing one selected provider, including explicitly configured secret-free access; distinct from credential lifecycle.
- **Configured-auth status:** Non-secret configuration metadata, not live credential validation or a prediction of request success.
- **PKCE:** Proof Key for Code Exchange: a random verifier paired with a SHA-256 challenge of its encoded text.
- **Token exchange:** An optional provider primitive invoked by the credential owner, not coordination of credential persistence, precedence or refresh.
- **Reported usage:** Provider-supplied, non-overlapping token categories for one attempt.
- **Flat rate:** A supplied USD-per-million price for one token category.
- **Cost estimate:** Arithmetic using reported categories and supplied rates, not a bill or balance.
- **Attempt:** One try at fulfilling a request; failed assistant attempts remain in raw history but are omitted from retry projection.
- **Model event:** An ordered update or terminal outcome emitted by a model stream, distinct from the agent event that wraps it.
- **Content block:** An indexed text, thinking or tool-call part of one assistant response.
- **Cancellation:** Stopping local request work or waiting with an aborted outcome, not undoing remote effects. The shared signal belongs to `maestro-cancellation`; only models and the terminal toolkit may depend on it, and it depends only on `maestro-listeners`, which keeps each signal's synchronous abort observers.
- **Scripted simulator:** An opt-in protocol adapter with queued responses, builders, chunking, pacing, usage/cache simulation and request-observing factories.

- **Requested thinking:** The application's supplied effort choice.
- **Effective thinking:** The supported choice used for this request.
- **Model descriptor:** Caller-supplied invocation data, independent of catalog membership.

## Application foundation

- **Settled:** The point after a run when its retries, compaction, summaries, persistence and queued continuations have all finished. A session is idle only then. See [the run](docs/application/lifecycle.md#the-run).
- **Hosted-service origin:** The single supplied origin for release checks, install reports and session-viewer links; an empty value disables those operations. It is separate from model-request attribution. See [hosted services](docs/application/services.md).

From `docs/specs/maestro-port.md`. Manifest mode is specified in
[manifest mode](docs/specs/manifest-mode.md).

### Manifest configuration

- **Manifest:** An organization's pinned configuration release named by the
  `manifest` setting; distinct from a package manifest and a detection manifest.
- **Package manifest:** The `maestro` key of a package's `package.json` listing its
  resources.
- **Internal mode:** Running without a manifest; foundation behaviour.
- **Manifest mode:** Running with a named manifest, which adds a layer and a policy.
- **Manifest package:** The default-on base package that selects, installs, reads
  and enforces a manifest through the core hooks.
- **Manifest root:** The one folder of a release, `manifests/`, that holds all of
  its manifest content. Maestro reads manifest content only from it; documentation,
  schemas and CI stay outside.
- **Manifest area:** The manifest root itself (the common area) or one folder under
  it (standard, core, language or capability) whose content enters the manifest
  layer in a fixed order.
- **Capability:** A manifest area that is one package bundling a team's roles,
  skills, instructions, prompt templates and workflows, selected by name.
- **Capability group:** The folder that groups capabilities, usually after the
  organization's team structure.
- **Authored artifact:** A file a user, project or manifest writes to configure or
  drive Maestro, such as a role, skill or workflow; distinct from a browser
  artifact.
- **Artifact kind:** The type of an authored artifact, with one format valid
  locally and in a manifest.
- **Workflow:** A named chain or parallel set of steps in a `maestro.workflow/v1`
  file.
- **Guard:** A manifest rule that blocks, confirms, redacts or warns on a command, a
  path or text, from a `maestro.guards/v1` file in the manifest's core area.
- **Guard family:** The set of guard rules in one guard file, named by the file,
  such as personal data or destructive commands.
- **Base layer:** A read-only settings document with locked keys, published by a
  package below the user layer.
- **Blueprint:** The catalog's description of one artifact kind: its properties
  and the relations its entries have.
- **Requires:** The catalog relation from a release or package to a package it
  needs. Skills and workflows inherit their package's requirements; selecting an
  entry installs the transitive required-package closure in its scope.
- **Policy:** The combined locks, bounds, allowed models, required entries, role
  constraints and release rules of a manifest; the manifest package decides every
  governed change.
- **Lock:** A policy entry that fixes a value or refuses local additions of a
  resource kind. A lock never loosens another rule.
- **Lock name:** A `locked` entry that names something other than a settings key:
  the prompts, the providers or a resource kind that has no settings key.
- **Bound:** A policy minimum or maximum for a numeric value or thinking level.
- **Required resource:** A manifest resource that local layers cannot disable,
  shadow or drop.
- **Minimum release:** The lowest manifest release a session may run; a release
  below it is updated at the next session start.
- **Permission growth:** A manifest release change that adds role tools, widens
  allowed models or a bound, removes a lock or required entry, or adds an
  extension, package or MCP server.

### Execution

- **Agent:** The conversation runner, not a supervisor persona.
- **Execution affinity:** The compile-time local or thread-safe representation of agent callables and submitted futures; sessions, including those the embedding SDK creates, run local work on a caller-owned local executor; standalone tool definitions default to thread-safe execution on native targets; on browser targets the default family is single-thread.
- **Run:** One foreground prompt or continuation through low-level idle.
- **Turn:** One assistant response with its tool results.
- **Tool:** An executable definition with preparation, input validation, execution, progress and preview/render callbacks.
- **Steering:** Input delivered at a turn boundary.
- **Follow-up:** Input delivered after ordinary continuation and steering.
- **Run handle:** An awaitable observation, not the owner or cancellation control of a run.
- **Batch:** The tool calls belonging to one assistant response.

### Conversation

- **SessionManager:** The owner of session entries, branch position, labels, resolved context and JSONL persistence.
- **Session:** A conversation history with a selected branch, backed by a native JSONL file or explicit memory storage.
- **AgentSession:** The application session coordinating agent execution, prompt delivery, events, selection, compaction, retries and navigation.
- **Entry:** An append-only item in the session tree, such as a message, label, model/thinking change or compaction record. The [maestro-entry library](docs/entry.md), which depends only on `maestro-request`, owns the session interchange records, including generic compaction results, `ForkPosition` and `NavigateTreeOptions`; `maestro-session` keeps history and storage.
- **Active branch:** The path from the root to the selected position.
- **Context projection:** The selected-ancestry message and setting values produced from supplied session entries; persistence remains with the session owner.
- **Compaction:** Reduction of model context without deleting raw history.

### State

- **Authored path:** A path string retaining the spelling supplied by a user, setting or file; `maestro-path` operates on it lexically, and it becomes a `std::path::PathBuf` only at a file-system call.
- **Transcript bytes:** Opaque bytes accessed at caller-supplied paths without parsing or framing.
- **Branch position:** The selected entry in the session tree, or the position before all entries.
- **Storage:** The replaceable raw supplied-path byte I/O interface; the SessionManager owns history and persistence timing.
- **Notification:** An observation of current activity; delivery does not imply persistence or replay.
- **Settings:** Raw global/project preference values, including unknown and wrong-typed properties; typed reads return owned copies with individual fallbacks.
- **Settings view:** An owned copy of a composite preference that keeps its stored object as written, with every member's type and position, while providing typed reads and in-place edits.
- **SettingsManager:** The owner of accepted preferences, one-level merging, runtime overrides, immediate publication and ordered queued persistence through replaceable raw-text storage. Reload independently retains failed scopes and clears overrides; errors drain separately.
- **Pending edit:** An unsaved preference edit captured with a revision; a successful save acknowledges only the revisions it wrote, so later or failed edits stay pending.
- **Configuration root:** The user configuration directory selected by the application configuration owner from a nonempty override or its home-based default; lower-level services consume the supplied root. See [application configuration](docs/application/configuration.md).
- **InstallMethod:** The package-manager classification selected from supplied installation layout and path evidence, without executing a manager.
- **SelfUpdateCommand:** An owned self-update command description with literal installation arguments, a display and optional ordered removal/installation steps; selection does not execute an update.
- **Session directory:** A caller-selected or default directory used for native session files.
- **File-write lock:** Native OS file locking on a stable sidecar file for mutual exclusion. The [shared effect contract](docs/specs/maestro-port.md#crates-and-delivery-order) assigns bounded synchronous acquisition of a caller-opened file to `maestro-lock`; settings and credentials retain opening, permissions, in-place persistence, unlocking and asynchronous acquisition policy. The [lock documentation](docs/locking.md) gives synchronous acquisition policy; the [credentials documentation](docs/credentials.md) gives the asynchronous schedule.
- **Credential:** Provider-owned authentication data, not permission to execute a tool.
- **Stored credential:** Local provider authentication data, distinct from already-resolved runtime request input. Its typed records belong to `maestro-request::auth_storage`; credentials re-exports them.
- **Account file:** The standalone `auth.json` an account command reads and writes at the caller's relative path, distinct from the session credential store; its entries hold stored credentials.
- **Secret helper:** An explicitly requested program whose output supplies a configured secret lazily.
- **AuthStorage:** The owner of accepted stored credentials, their ordered listing, drainable errors and per-provider persistence through a replaceable raw-text backend, plus request key selection, OAuth login and locked token refresh; the [credentials documentation](docs/credentials.md) describes them, and the provider registry and refresh operation belong to the [OAuth documentation](docs/models/oauth.md).

- **Browser store**: application-owned policy for a named collection behind an injected
  multi-collection backend; distinct from native raw-byte storage. See
  [browser collection storage](docs/application/browser-storage.md). Session histories
  and indexed session metadata occupy separate collections; `AppStorage` retains
  their supplied stores and application adapter.

### Extensibility

- **Extension:** Admitted behavior that owns declared capabilities.
- **EventBus:** Shared literal event channels with ordered listener snapshots, explicit unsubscribe and callback-scoped foreign delivery; asynchronous listener tails are independently owned.
- **Registration:** A host-admitted declaration of an owned capability.
- **Runtime:** Shared extension action slots, queued provider registrations and the existing activity/flag state. Facades and their runner retain the same runtime.
- **Actions:** Application-supplied extension operations selected at call time; an already-started model future retains its selected operation.
- **Extension UI context:** An owned handle to the UI host current when it was obtained; without an installed host its operations return the defined headless results.
- **Extension context:** The single-thread handle callbacks receive; each operation fails with the stale error once its runner is invalidated, otherwise it reads the runner's current session, UI and bound operations, except the model reader fixed at creation.
- **Command context:** The single-thread extension context plus guarded, call-time session-control operations; replacement contexts add the creating session's unguarded message operations.
- **Message renderer:** An extension callback that renders a custom message type; the first extension in load order that registered the type supplies it.
- **Browser message renderer:** A typed callback registered by an open message role. Registration replaces that role's current callback; already retained handles keep the prior callback. This browser replacement policy is separate from extension load order.
- **Page content:** Browser content that mounts its own DOM root once and updates that root in place while its content type stays; `maestro-web::dom::MountedContent` keeps one root per placement. See [page content](docs/browser/frontend.md#page-content).
- **Browser contribution:** A package-declared page, Home card, tab, tool or message renderer, or host API listed under `maestro.contributions`; its identity is its package plus its identifier, and its kind is extensible data. A slot mounts the contributions attached to its name through one caller. See [contributions](docs/browser/frontend.md#contributions).
- **Browser module:** A separately built WebAssembly module with its own binding glue that registers the contributions declared with its own module entry through the interface it receives when loaded; that load owns its registrations. Trusted modules run in the page; untrusted UI uses an isolated frame. See [runtime modules](docs/browser/frontend.md#runtime-modules).
- **Package:** An npm, Git or local source containing extension components or resources; consumer installation does not compile Rust code.
- **File pattern:** A glob or ignore rule matched against relative file paths.
- **Resource:** Discovered instruction material, such as skills, prompt templates or context files, and the extension entry points a package manifest or index file declares, with source/provenance data and entry-point-specific ordering.
- **Context file:** An `AGENTS.md` or `CLAUDE.md` file from the agent directory or a directory from the filesystem root down to the working directory, included in the system prompt. See [application resources](docs/application/resources.md#context-files).
- **Session services:** Credentials, settings, the model catalog and reloaded resources bound to one session working directory, created before and separately from the session. See [application resources](docs/application/resources.md#session-services).
- **Module loader:** The caller-supplied operation that turns a resolved extension path into a factory; the component runtime supplies it and the executable wires it.

Tool parameter schemas have one owner, `maestro-request::parameter_schema::ParameterSchema<C = ()>`,
which keeps the supplied JSON Schema value beside declared node kinds and a generic
executable payload. Its data-only default stays usable in model requests. Shared
`Base`, `Refinement` and `SchemaNode` records retain local author callbacks; guest
specializations use borrowed JSON operands on the existing callback resources. `Tool`, `Context` and `ToolInfo` default to that owner, the guest
re-exports it, and the model argument-validation owner is its only converter.
Serialization writes the supplied JSON alone.

Tool metadata has one generic record owner, `maestro-request::tool_info::ToolInfo<P>`.
Native callers use the default parameter-schema owner through models; guest authoring
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

- **Registered tool:** A retained typed definition with shared provenance in the
  extension's ordered owner. Callback ownership follows the facade; invocation is
  local with a fresh concrete context. SDK standalone defaults remain threaded.
- **Shared extension events:** Generic event/result schemas and assembled prompt
  records owned by request, re-exported by models and specialized by native/guest
  aliases; adapter capabilities and stream snapshots remain adapter-owned.

### Interfaces

- **Footer metadata:** A cached Git branch, extension status texts and a supplied available-provider count, owned by `maestro-app` and rendered by frontends. Its read-only contract and live status view belong to `maestro-extensions`, whose footer factories receive them.
- **Application:** The shared operations and embedding SDK exposed by `maestro-app`; frontends select presentation, not alternate application policy. Its skill-block parser of an expanded prompt belongs to the application; the native transcript reads it, and it is available to browser builds, though the offline viewer does not read it yet.
- **CLI:** The command-line frontend for argument parsing, startup and print/JSON operations.
- **Chat:** The interactive terminal frontend, including transcript presentation, editor and selectors.
- **RPC:** The JSONL frontend with correlated responses, stream events and the established command/UI methods.
- **RPC client:** The typed embedding client in `maestro-app` that launches Maestro in RPC mode as a child process and sends it the session commands; it owns the child's I/O, stderr, request correlation and its own deadlines.
- **Web:** The browser frontend, including the offline exported session viewer.
- **Brand pack:** One JSON file of palette, mode role aliases, fonts, type, spacing, mark paths, glyphs and terminal aliases; [the brand library](docs/brand.md) owns its pure admission and projection, while [the theme library](docs/theme.md#brand-pack) owns native pack/template loading and terminal theme documents. Callback pages consume the same brand owner with embedded assets.
- **Theme:** The shared presentation library for style resolution and caller-specific code highlighting. A *prepared instance* is an immutable set of color prefixes; the *live theme* is the shared slot that publishes the current instance and may be replaced while callers still hold earlier instances. Component color callbacks read the live theme when called (strikethrough and unselected settings labels do not); border callbacks keep the instance they were created from and the settings cursor keeps the color it had at creation. The [shared effect contract](docs/specs/maestro-port.md#crates-and-delivery-order) assigns watch/timer effects to `maestro-watch::fs_watch`, leaving theme reload policy here.

### Structure

Internal-dependency descriptions below state domain edges, subject to the
[native feature-unifier exception](docs/specs/maestro-port.md#native-feature-unifier),
including leaves and exact sets. The unifier itself has no workspace dependencies.

- **Catalog:** The application-facing usable model selection resolved from models and credentials.
- **Tools:** The owner of executable tool definitions and their caller-supplied preview/render context. The [tool-record library](docs/tool-records.md), `maestro-tool`, depends only on `maestro-request` and owns the tool interchange records; `maestro-tools` keeps execution.
- **Export:** Session document serialization shared with application operations.
- **Terminal toolkit:** Terminal components in `maestro-tui` depending internally only on shared cancellation, the text leaf, the native process leaf, the shared Markdown parser and the optional foundation utility, without application selectors or framework/highlighting engines.
- **Frame writer:** The `TUI` owner in `maestro-tui` that draws components to a terminal as retained frames, routes terminal input and schedules renders through its runtime.
- **Overlay:** A retained component layered over the frame, with live placement/visibility options and a public handle for focus, temporary hiding and removal.
- **Retained frame:** The lines the terminal currently shows, kept so the next update writes only the rows that changed.
- **Terminal runtime:** The host effects a frame writer asks for: monotonic time, deferred callbacks, local future execution, environment queries and log files.
- **Terminal adapter:** The real-terminal connection in `maestro-tui-crossterm`, whose normal/build dependencies are limited to the toolkit, the text leaf and, optionally, the foundation utility; it requires the toolkit and text directly. Its examples may use the brand owner through the permitted terminal-example development edge; that edge does not satisfy either runtime requirement.
- **File mutation queue:** The native per-file ordering in `maestro-tools` that runs file-changing tool operations on one file one at a time, in call order, while different files proceed independently; files are identified by canonical path, or by lexical absolute path when canonicalization fails.
- **OutputAccumulator:** A native tool-output stream that owns decoded snapshot accounting and an optional raw overflow artifact.
- **ToolsManager:** The native owner of the fd and rg search programs in maestro-tools: it looks in the supplied binary directory and on PATH, then applies call-time offline and Android policy before attempting a missing program's latest-release installation.
- **Grapheme:** A user-perceived character cluster, indivisible when terminal text is wrapped, truncated or sliced; escapes inside it never split it. Tool-output line clipping instead counts Unicode scalars.
- **Cell column:** One terminal cell of width; text widths count cells, and a tab counts three in measured text but none in column selection.
- **Image protocol:** The terminal's Kitty or iTerm2 inline image transport, distinct from image file format.
- **Terminal capabilities:** Cached support for inline images, true color and hyperlinks for one terminal.
- **Cell dimensions:** Positive pixel width and height of a terminal cell, used to reserve image rows.
- **Byte cursor:** A cursor position counted in UTF-8 bytes within a line, never in cells or characters.
- **Key identifier:** Open text naming one key and the modifiers held with it, such as `ctrl+c` or `shift+tab`; terminal input is matched against it or parsed into it.
- **Keybinding/action:** An open terminal action ID resolved to zero or more literal key identifiers.
- **Keybinding definition:** Default keys and an optional description for an action.
- **User binding:** A supplied action override; an unset value uses defaults and an empty list disables the action.
- **Keybinding conflict:** Multiple distinct known actions explicitly claiming the same literal key, in claim order.
- **Enhanced keyboard protocol:** The terminal mode that reports keys as escape sequences carrying exact modifiers, alternate layouts and press, repeat and release events; one process-wide flag records whether it is active.
- **Input buffer:** The `StdinBuffer` of `maestro-tui`, which frames raw terminal input chunks into ordered data and paste events and owns no clock; its caller supplies each input's instant and expires the buffer at its deadline.
- **Fragment:** An incomplete escape sequence the input buffer retains for more input; it can be completed, explicitly flushed or cleared, released as it is at a representable deadline, or delivered as data immediately before a paste starts.
- **Bracketed paste:** Text a terminal wraps in start and end markers; the input buffer delivers it as one paste event without the markers.
- **Terminal scenario harness:** The dedicated `maestro-test-terminal` scenario runner whose library owns the recording terminal and the manual runtime, declared as a native development dependency by `maestro-cli` only.
- **Guest authoring:** The `maestro-extensions-wasm` library, whose allowed internal dependencies are the request, entry, tool, JSON and text owners (never native session, tool or catalog runtimes), owning the canonical WIT source inputs and the facade extension authors write against.
- **Shared request owner:** The core `maestro-request` library below layer 0, whose job is to define shared message and model-request records, including model/message/content/usage/stream/diagnostic records, `SourceInfo`, `Skill`, `SlashCommandInfo`, `SlashCommandSource`, `MessageRenderOptions`, `CustomMessageInput<T>`, `MessageDelivery`,
`UserMessageDelivery`, `SendMessageOptions` and `SendUserMessageOptions`, the four application message records and their existing `Presence<T>` wrapper; it also owns `model_registry::ResolvedRequestAuth`, re-exported by the native catalog and guest authoring library, default zero usage, flat-rate `calculate_cost` arithmetic, `DiagnosticErrorInfo` as the one thrown-error form with its message-only construction, and the shared optional-field helper; only the crates that list it in the spec table (models, resources, theme, catalog, session, entry, tool and guest authoring) may depend on it directly, and it depends only on the JSON leaf.
- **Extension JSON wire:** Plain serde_json payloads beside typed capabilities; finite numbers retain their value and signed zero, ordinary host nonfinite values serialize as null, and extension-written Infinity or NaN fails clearly before output. Shared records have no bit-number encoding or added Presence wrapper.
- **Runtime adapter:** The replaceable artifact executor in `maestro-extensions-wasmtime`, whose permitted internal targets are extensions and the optional native path utility.
- **Attachment:** Application-owned original bytes, media metadata and optional derived text/preview, with browser-message conversion. See [browser attachments](docs/application/browser-attachments.md).
- **Text:** The core leaf `maestro-text` below layer 0, owning the whitespace scalar set, UTF-16 lexical order, one-pass markup escaping, CSS string serialization, terminal escape stripping and binary-output filtering, raw CR/LF byte-line framing, BOM-aware lossy UTF-8 decoding and, behind its `collation` and `number-format` features, locale comparator and number-formatter preparation through one locale selector; it depends on no workspace crate and its callers keep their own trimming, stable sorting and comparator lifetime. See [text](docs/text.md).
- **JSON boundary:** The declared core leaf `maestro-json` below layer 0, owning JSON boundary semantics (object acceptance, canonical keys, truth, compact and pretty text, JSONL line framing); it depends on no workspace crate and its callers keep their own preprocessing, defaults, duplicate-member policy and error wording. See [JSON boundary](docs/json.md).
- **Process:** The declared core leaf `maestro-process` below layer 0, owning native child-process lifecycles (captured spawn, raw pipes, exit and EOF observation, termination and reaping, Linux pane terminals and their shared retained-front byte queue, and native termination-signal delivery with its prepared default action) for packages, credentials, detection, the application, the terminal toolkit, the terminal adapter, the daemon and tools; it depends on no workspace crate, including the path and cancellation leaves. Its [contract](docs/specs/maestro-port.md#crates-and-delivery-order) defines synchronous capture's timeout handling and leaves cancellation, termination and settlement choices with callers.
- **Test process:** The `maestro-test-process` crate, a dedicated, unpublished, native, std-only test-support leaf that owns one named test's isolated execution: its checked-output operations start a libtest child and prove the child started exactly that test; argument-only invocation checks eligibility and leaves startup proof with the caller; distinct from product process execution (**Process**). It is reached only through native development dependencies of its declared callers. See the [contract](docs/specs/maestro-port.md#test-process-owner).
- **Foundation utility:** The `maestro-path` library of lexical path operations, native component normalization, home and directory selection from caller-supplied observations, and the native environment-plus-account home reader, below every delivery layer; native crates may depend on it optionally, except those the [crate graph](docs/specs/maestro-port.md#crates-and-delivery-order) excludes, and it depends on no other workspace crate.
- **Directory listing:** The core leaf `maestro-listing` below layer 0, owning one directory's entries sorted by file name on every platform; resources, theme, the terminal toolkit, tools, session and the application list directories through it on native targets and keep their own kind filtering, name decoding and further ordering. Guest-authoring examples also use it through a portable development dependency from `maestro-extensions-wasm`, never a normal or build dependency. It depends on no workspace crate. See the [crate graph](docs/specs/maestro-port.md#crates-and-delivery-order).
- **Live listener collection:** The core leaf `maestro-listeners` below layer 0, owning callbacks in live registration order with one admission per identity and removal that returns ownership; models, the toolkit, agent, application, extensions and cancellation subscribe and walk through it, and each keeps its own delivery and settlement policy. It depends on no workspace crate. See the [crate graph](docs/specs/maestro-port.md#crates-and-delivery-order).
- **Recoverable publication:** `maestro-lock` locks a canonical target, durably stages its intended bytes, then writes and syncs the target in place before deleting the recovery copy. The next holder replays an interrupted write. Native ownership, ACLs and hard links survive. Workspace owns snapshot serialization and chooses link following. See [locking](docs/locking.md#recoverable-byte-publication).
- **Dependencies:** The generated `maestro-dependencies` crate for dependency feature unification in the [configured native scope](docs/development.md#feature-unification), including its traversal exclusions; it exports no API, has no workspace dependencies and is not reachable through wasm32 dependency edges.
- **Conventions:** Native workspace graph and bounded source/build checks, supplemented by semantic source review.
- **Tooling:** Development-only commands and repository automation for this repository; never shipped.

- **Input:** The single-line editable terminal component.
- **Editor:** The multiline editable terminal component.
- **Kill ring:** Shared deleted-text history for yank and rotation.
- **Undo stack:** Shared clone-on-push snapshots for restoring editable state.

- **SelectList:** A terminal command-selection component.
- **SettingsList:** A terminal settings component with optional label search.

- **Utf8Stream:** The text leaf's raw streaming decoder; callers own BOM selection and framing.

- **BashExecutorOptions:** Optional cleaned-chunk callback and signal for direct shell execution.
- **BashResult:** Direct shell selected text, numeric status, cancellation and artifact observations.
- **Artifact:** A mutable dated file retained by the browser application command owner; its ordered map and record handles can outlive a command. See [artifact commands](docs/application/artifacts.md).

- **Workbook:** The application-owned ordered worksheet collection, numeric date epoch and detached number-format table snapshot returned by the XLSX/XLS reader. See [spreadsheet values and workbooks](docs/application/document-formats.md).
- **Worksheet:** Ordered populated spreadsheet cells with retained raw values, optional formatted text, generated cell markup, raw format strings, literal hyperlink targets, declared or observed range and ordered merged regions.

- **Browser input:** The retained `maestro-web::Input` field, with native browser editing and event/ref handles; distinct from the terminal toolkit `Input`.
- **Reading content:** A feature-owned native text factory admitted by the existing browser attachment-surface registry; the shared host retains a fixed placement frame, one host-created clipped scrolling viewport and the feature root, measures natural height and frame-local canvas-control slots, restores optional styles and fractional viewport scroll transactionally, and retains unchanged roots. See [owned reading content](docs/browser/attachments.md#owned-reading-content).

- **Browser SandboxIframe:** The retained loading/execution adapter in `maestro-web`. One native frame owner serves direct execution and the hidden application host, with retryable removal and native result handles; preparation can produce standalone documents. See [sandbox execution](docs/browser/sandbox.md#hidden-javascript-execution).

- **Markdown parser:** `maestro-markdown` owns native structure, source extents and cursor-time extended links; application exposes it as `markdown`, while adapters retain presentation policy. Native authored text and quote bodies use owned, shared `MappedText` values with chunk reads and explicit string materialization.

## Workspace daemon

- **Action manifest:** A file declaring one identifier's workspace actions, action panes, event hooks, link handlers, build steps and startup steps, delivered as package content of the `actions` resource kind. It is distinct from a package manifest, a detection manifest and an organization manifest.

- **Workspace daemon:** The long-lived Maestro process owning workspaces, tabs, panes, pane terminals and pane agents for one user; closing a client never stops its work.
- **Daemon state:** The daemon-owned record of workspace topology, terminal metadata, focus, notifications and pending persistence. Live process handles stay in the application runtime registry.
- **Daemon loop:** The application loop that drains internal observations, consumes scheduled work, selects the next deadline and completes shutdown.
- **Named daemon:** An independent workspace daemon with its own sockets and snapshot, chosen by name.
- **Daemon protocol:** Maestro's single versioned RPC contract shared by stdio, daemon sockets and SDK clients. `maestro-protocol` owns the generic envelopes, the workspace and binary-channel records and JSON Schema; `maestro-app::rpc_types` owns the session command, response, event and dialog records; `maestro-json::jsonl` owns JSONL framing. Live screen frames are binary within that contract. See [daemon protocol](docs/daemon-protocol.md).
- **Workspace:** A named project space in the daemon holding ordered tabs; not the Cargo workspace.
- **Tab:** An ordered split layout of panes within a workspace.
- **Pane:** One leaf of a tab's layout, showing one pane terminal.
- **Terminal emulator:** The pure-Rust engine in `maestro-terminal` that parses a pane terminal's output, keeps its screens and byte-bounded scrollback, and encodes its keyboard, mouse and focus input; see the [terminal emulator specification](docs/specs/terminal-emulator.md).
- **Terminal effects:** PTY reply bytes, bell counts, directory changes and clipboard writes captured by the terminal emulator for its caller to drain after a write.
- **Render state:** The emulator's retained viewport snapshot with whole-screen and per-row dirty flags.
- **Image placement:** An image's position on a pane terminal's grid, given directly or through placeholder cells.
- **Host graphics cache:** The multiplexer-side record of images uploaded to the host terminal and the placements shown there. See [host graphics](docs/terminal-emulator.md#host-graphics).
- **Scrollback limit:** The bytes of page storage a pane terminal keeps beyond its active screen; zero disables history.
- **Pane terminal:** A PTY child with its terminal emulator, scrollback and metadata.
- **Pane runtime:** The daemon's live object for one pane terminal: the PTY child, its emulator and its agent detection task.
- **Pane agent:** A coding agent recognized in, or reporting from, a pane, with status idle, working, blocked, done or unknown; distinct from **Agent**, the conversation runner.
- **Detection catalog:** The downloaded, version-checked cache of detection manifests, used ahead of the bundled set when newer.
- **Detection manifest:** Bundled or downloaded data rules recognizing an agent and its status from a pane's screen, title or progress reports.
- **Workspace action:** A declarative command, pane or hook delivered as package content and run by the daemon's command runner.
- **Multiplexer:** The full-screen frontend attaching to a workspace daemon to show its frames.
- **Attach / detach:** Connecting a multiplexer client to a daemon, and leaving the daemon running.

## Packages

From `docs/specs/packages.md`.

- **Package crate:** A guest-only reusable `rlib` exposing `factory`, with one
  component export in `src/component.rs`. Its `component` example builds as a
  `cdylib`. Packages may use the guest API, shared request/entry/tool/JSON records
  and other packages; core and dedicated crates cannot depend on packages.

- **Skill pack:** a content-only package of skills and the shared reference files they read; Maestro's own is
  `maestro-skills`.
- **Type-only skill:** a skill marked `disable-model-invocation: true`; it stays out of the model's skill list and
  runs only when typed as `/skill:<name>` or through an alias command.

## Tool and terminal additions

From `docs/specs/tool-additions.md`.

- **Output backlog:** Bytes the native terminal has queued and not yet written.

- **Test directory:** The dedicated, unpublished, native `maestro-test-directory`
  leaf that owns exclusive disposable-root allocation and cleanup. Automatically
  chosen roots are canonical; supplied parents retain native joining semantics.
  On Unix it also enters removed or unreadable working directories for tests that
  run alone. Only the callers its [contract](docs/specs/maestro-port.md#test-directory-owner)
  declares may use it, through native development edges.

- **Test fixtures:** The dedicated, unpublished, portable `maestro-test-fixtures`
  leaf for shared fixture records, header lookup, wire-literal round trips and
  single-color theme construction. Its eight callers use development edges only;
  it has no product or utility dependency. See the
  [contract](docs/specs/maestro-port.md#test-fixture-owner).
- **Test tracing:** The dedicated, unpublished, native `maestro-test-tracing`
  leaf that runs a test action under a scoped tracing subscriber that sees
  every callsite, whichever thread reaches the callsite first. Only daemon,
  models, process, terminal and workspace may use it through native development
  edges, and tests capture only through it.
  See the [contract](docs/specs/maestro-port.md#test-tracing-owner).

The parameter-schema owner also owns the component metadata transport in
`maestro-request::parameter_schema::transport`, re-exported through models and native extensions
as `schema_transport`. Guest registration and enumeration use that same codec;
ordinary provider and persisted JSON omit the carried schema kinds. This adds no
crate dependency: both extension adapters already reach the request owner through
their declared edges.

# Glossary

The implementation is being rebuilt crate by crate; these terms describe the
foundation contract, not currently delivered capabilities.

Add terms here when the specs establish their meaning; use one name per concept.

## Models

From `docs/specs/maestro-port.md`.

- **Provider:** A named source of model descriptors and authentication behavior; a protocol adapter owns invocation and wire conversion.
- **Model identity:** The combination of provider ID and model ID.
- **Replay identity:** Provider, API and model ID used to decide whether signed response content is reusable.
- **Protocol adapter:** An implementation of a registered model protocol, including provider-specific wire conversion and usage interpretation.
- **ModelRegistry:** The owner of the usable local model catalog, local overrides and dynamic provider registrations.
- **Tool arguments:** The model-supplied argument object, completed from accumulated JSON before execution.
- **Tool declaration:** A model-facing name, description and JSON Schema argument contract, distinct from an executable tool.
- **Model-request projection:** The selected-model view of supplied conversation context, not the stored branch or a history write.
- **Catalog override:** Partial nested metadata supplied by local model files over base catalog entries.
- **ApiProvider:** Ordered protocol registration holding raw and simple invocation callbacks.
- **EventStream:** Producer-owned FIFO with independently observable result.
- **AssistantMessageEventStream:** EventStream specialization retaining shared assistant-message handles.
- **Request:** An invocation of a supplied model descriptor with its inputs, options, authentication and cancellation; catalog membership is not required.
- **Request authentication:** Invocation data authorizing one selected provider, including explicitly configured secret-free access; distinct from credential lifecycle.
- **Configured-auth status:** Non-secret configuration metadata, not live credential validation or a prediction of request success.
- **Token exchange:** An optional provider primitive invoked by the credential owner, not coordination of credential persistence, precedence or refresh.
- **Reported usage:** Provider-supplied, non-overlapping token categories for one attempt.
- **Flat rate:** A supplied USD-per-million price for one token category.
- **Cost estimate:** Arithmetic using reported categories and supplied rates, not a bill or balance.
- **Attempt:** One try at fulfilling a request; failed assistant attempts remain in raw history but are omitted from retry projection.
- **Model event:** An ordered update or terminal outcome emitted by a model stream, distinct from the agent event that wraps it.
- **Content block:** An indexed text, thinking or tool-call part of one assistant response.
- **Cancellation:** Stopping local request work or waiting with an aborted outcome, not undoing remote effects.
- **Scripted simulator:** An opt-in protocol adapter with queued responses, builders, chunking, pacing, usage/cache simulation and request-observing factories.

- **Requested thinking:** The application's supplied effort choice.
- **Effective thinking:** The supported choice used for this request.
- **Model descriptor:** Caller-supplied invocation data, independent of catalog membership.

## Application foundation

From `docs/specs/maestro-port.md`. Manifest governance and settings value locks
are later scope, not active foundation concepts.

### Execution

- **Agent:** The conversation runner, not a supervisor persona.
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
- **Entry:** An append-only item in the session tree, such as a message, label, model/thinking change or compaction record.
- **Active branch:** The path from the root to the selected position.
- **Context projection:** The model-bound view of supplied conversation context; branch selection and storage remain session responsibilities.
- **Compaction:** Reduction of model context without deleting raw history.

### State

- **Authored path:** A path string retaining the spelling supplied by a user, setting or file; `maestro-path` operates on it lexically, and it becomes a `std::path::PathBuf` only at a file-system call.
- **Transcript bytes:** Opaque bytes accessed at caller-supplied paths without parsing or framing.
- **Branch position:** The selected entry in the session tree, or the position before all entries.
- **Storage:** The replaceable raw supplied-path byte I/O interface; the SessionManager owns history and persistence timing.
- **Notification:** An observation of current activity; delivery does not imply persistence or replay.
- **Settings:** Raw global/project preference values, including unknown and wrong-typed properties; typed reads return owned copies with individual fallbacks.
- **Settings view:** An owned copy of a composite preference that preserves raw members for writes while providing typed reads and local edits.
- **SettingsManager:** The owner of accepted preferences, one-level merging, runtime overrides, immediate publication and ordered queued persistence through replaceable raw-text storage. Reload independently retains failed scopes and clears overrides; errors drain separately.
- **Configuration root:** The explicitly selected user configuration directory.
- **Session directory:** The resolved location supplied to the session owner.
- **File-write lock:** Native OS file locking for mutual exclusion, preserving application-owned ordering and retries without leases, heartbeats or compromise handling.
- **Credential:** Provider-owned authentication data, not permission to execute a tool.
- **Stored credential:** Local provider authentication data, distinct from already-resolved runtime request input.
- **Secret helper:** An explicitly requested program whose output supplies a configured secret lazily.
- **Read-only credential storage:** A storage adapter that permits reads but rejects every replacement; it does not contain helper effects.

### Extensibility

- **Extension:** Admitted behavior that owns declared capabilities.
- **Registration:** A host-admitted declaration of an owned capability.
- **Package:** An npm, Git or local source containing extension components or resources; consumer installation does not compile Rust code.
- **Resource:** Discovered instruction material, such as skills, prompt templates or context files, with source/provenance data and entry-point-specific ordering.

### Interfaces

- **Application:** The shared operations and embedding SDK exposed by `maestro-app`; frontends select presentation, not alternate application policy.
- **CLI:** The command-line frontend for argument parsing, startup and print/JSON operations.
- **Chat:** The interactive terminal frontend, including transcript presentation, editor and selectors.
- **RPC:** The JSONL frontend with correlated responses, stream events and the established command/UI methods.
- **Web:** The browser frontend, including the offline exported session viewer.
- **Theme:** The shared presentation library for style resolution and caller-specific code highlighting.

### Structure

- **Catalog:** The application-facing usable model selection resolved from models and credentials.
- **Tools:** The owner of executable tool definitions and their caller-supplied preview/render context.
- **Export:** Session document serialization shared with application operations.
- **Terminal toolkit:** Terminal components in `maestro-tui` with no internal dependency beyond the foundation utility, without application selectors or framework/highlighting engines.
- **Terminal adapter:** The real-terminal connection in `maestro-tui-crossterm`, depending directly only on the toolkit and, optionally, the foundation utility.
- **Grapheme:** A user-perceived character cluster, indivisible when text is wrapped, truncated or sliced; escapes inside it never split it.
- **Cell column:** One terminal cell of width; text widths count cells, and a tab counts three in measured text but none in column selection.
- **Image protocol:** The terminal's Kitty or iTerm2 inline image transport, distinct from image file format.
- **Terminal capabilities:** Cached support for inline images, true color and hyperlinks for one terminal.
- **Cell dimensions:** Positive pixel width and height of a terminal cell, used to reserve image rows.
- **Byte cursor:** A cursor position counted in UTF-8 bytes within a line, never in cells or characters.
- **Key identifier:** Open text naming one key and the modifiers held with it, such as `ctrl+c` or `shift+tab`; terminal input is matched against it or parsed into it.
- **Enhanced keyboard protocol:** The terminal mode that reports keys as escape sequences carrying exact modifiers, alternate layouts and press, repeat and release events; one process-wide flag records whether it is active.
- **Terminal scenario harness:** The dedicated `maestro-test-terminal` scenario runner, not a reusable internal dev-dependency target.
- **Guest authoring:** The dependency-free `maestro-extensions-wasm` library owning canonical WIT source inputs.
- **Runtime adapter:** The replaceable artifact executor in `maestro-extensions-wasmtime`, whose only permitted internal target is extensions.
- **Foundation utility:** The `maestro-path` library of lexical path operations, below every delivery layer; native crates may depend on it optionally, and it depends on no other workspace crate.
- **Conventions:** Native workspace graph and bounded source/build checks, supplemented by semantic source review.
- **Tooling:** Development-only commands and repository automation for this repository; never shipped.

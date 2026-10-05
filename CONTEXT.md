# Glossary

Add terms here when the specs establish their meaning; use one name per concept.

## Models

From `docs/specs/2026-10-04-models.md`.

- **Provider:** A registered adapter that supplies models and implements declared operations using supplied request authentication.
- **Model identity:** The combination of provider ID, model ID and operation.
- **Operation:** A kind of model work: chat, classification, image generation, embedding or reranking.
- **Catalog:** The locally materialized collection of model entries with their identities, capabilities and flat price metadata.
- **Tool declaration:** A model-facing name, description and JSON Schema argument contract, distinct from an executable tool.
- **Model-request projection:** The selected-model view of supplied conversation context, not the stored branch or a history write.
- **Catalog override:** Reversible supplied metadata over base registrations, keyed by provider and complete model identity.
- **Captured model:** The effective registered metadata retained by one request despite later registry changes.
- **Request:** An invocation of a selected model operation with its inputs, options, authentication and cancellation.
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
- **Scripted fake:** A provider adapter driven by queued request responses or request-inspecting factories, with observable calls and no external I/O; queued scripted steps are responses, not deferred jobs.

- **Requested thinking:** The application's supplied effort choice.
- **Effective thinking:** The supported choice used for this request.
- **Request capability:** A registered declaration of supported request behavior.

## Engine

From `docs/specs/2026-10-05-engine-core.md`.

### Execution

- **Agent:** The conversation runner, not a supervisor persona.
- **Run:** One foreground prompt or continuation through low-level idle.
- **Turn:** One assistant response with its tool results.
- **Tool:** A registered callable capability with validated input and an explicit outcome.
- **Steering:** Input delivered at a turn boundary.
- **Follow-up:** Input delivered after ordinary continuation and steering.
- **Run handle:** An awaitable observation, not the owner or cancellation control of a run.
- **Batch:** The tool calls belonging to one assistant response.
- **Preparation:** Cancellable current-format argument transformation before shared validation and before hooks.
- **Finalized tool result:** Executed or rejected output after accepted progress and applicable after hooks settle, with a separate error flag.
- **Termination hint:** Optional runtime-only tool output flag suppressing ordinary continuation only when every finalized result in a nonempty batch is true.

### Conversation

- **Session:** A conversation history with a selected branch.
- **Entry:** An immutable record in the session tree.
- **Active branch:** The path from the root to the selected position.
- **Context projection:** The model-bound view of supplied conversation context; branch selection and storage remain session responsibilities.
- **Compaction:** Reduction of model context without deleting raw history.

### State

- **Record:** Opaque caller-owned bytes with a session-local lookup identity.
- **Selected position:** A record identity, or the position before all records.
- **Storage handle:** A shareable view permanently bound to one session identity, with its own admission and close lifecycle.
- **Definite rejection:** An operation outcome guaranteeing the prior state is unchanged.
- **Uncertain write outcome:** A mutation whose publication is unknown, preventing safe continued mutation through that handle.
- **Storage:** The replaceable interface that records session state; ephemeral storage is not the memory capability.
- **Notification:** An observation of current activity; delivery does not imply persistence or replay.
- **Settings:** Effective configured values with origins and enforceable value locks. A stored scope is user or project configuration; an ephemeral override changes effective values only until reload or a stored update.
- **Setting origin:** The source of an effective value.
- **Value lock:** A manifest constraint freezing a value or subtree, not a filesystem lock.
- **Configuration root:** The explicitly selected user configuration directory.
- **Session directory:** The resolved location supplied to the session owner.
- **File-write lock:** Native transaction exclusion for cooperating settings writers, distinct from a manifest value lock.
- **Credential:** Provider-owned authentication data, not permission to execute a tool.
- **Stored credential:** Local provider authentication data, distinct from already-resolved runtime request input.
- **Secret helper:** An explicitly requested program whose output supplies a configured secret lazily.
- **Read-only credential storage:** A storage adapter that permits reads but rejects every replacement; it does not contain helper effects.
- **Manifest:** Governed declarations of defaults, packages and resources consumed by their owners.

### Extensibility

- **Extension:** Admitted behavior that owns declared capabilities.
- **Registration:** A host-admitted declaration of an owned capability.
- **Package:** An installed source containing declared extension programs or resources.
- **Resource:** Instruction material or documentation made discoverable to the engine.

### Interfaces

- **Application:** The session operations shared by embedding and frontends. _Avoid_: app.
- **CLI:** The command-line frontend, including chat, print, JSON and RPC modes.

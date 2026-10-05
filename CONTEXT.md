# Glossary

Add terms here when the specs establish their meaning; use one name per concept.

## Models

From `docs/specs/2026-10-04-models.md`.

- **Provider:** A registered adapter that supplies models and implements declared operations using supplied request authentication.
- **Model identity:** The combination of provider ID, model ID and operation.
- **Operation:** A kind of model work: chat, classification, image generation, embedding or reranking.
- **Catalog:** The locally registered collection of model entries with their identities, capabilities and flat price metadata.
- **Request:** An invocation of a selected model operation with its inputs, options, authentication and cancellation.
- **Attempt:** One try at fulfilling a request; failed assistant attempts remain in raw history but are omitted from retry projection.
- **Model event:** An ordered update or terminal outcome emitted by a model stream, distinct from the agent event that wraps it.
- **Content block:** An indexed text, thinking or tool-call part of one assistant response.
- **Cancellation:** Stopping local request work or waiting with an aborted outcome, not undoing remote effects.
- **Scripted fake:** A provider adapter driven by queued request responses or request-inspecting factories, with observable calls and no external I/O; queued scripted steps are responses, not deferred jobs.

## Engine

From `docs/specs/2026-10-05-engine-core.md`.

### Execution

- **Agent:** The conversation runner, not a supervisor persona.
- **Run:** One foreground prompt or continuation through low-level idle.
- **Turn:** One assistant response with its tool results.
- **Tool:** A registered callable capability with validated input and an explicit outcome.
- **Batch:** The tool calls belonging to one assistant response.

### Conversation

- **Session:** A conversation history with a selected branch.
- **Entry:** An immutable record in the session tree.
- **Active branch:** The path from the root to the selected position.
- **Context projection:** The model-bound view of the active branch.
- **Compaction:** Reduction of model context without deleting raw history.

### State

- **Storage:** The replaceable interface that records session state; ephemeral storage is not the memory capability.
- **Notification:** An observation of current activity; delivery does not imply persistence or replay.
- **Settings:** Effective configured values with origins and enforceable value locks. A stored scope is user or project configuration; an ephemeral override changes effective values only until reload or a stored update.
- **Setting origin:** The source of an effective value.
- **Value lock:** A manifest constraint freezing a value or subtree, not a filesystem lock.
- **Credential:** Provider-owned authentication data, not permission to execute a tool.
- **Manifest:** Governed declarations of defaults, packages and resources consumed by their owners.

### Extensibility

- **Extension:** Admitted behavior that owns declared capabilities.
- **Registration:** A host-admitted declaration of an owned capability.
- **Package:** An installed source containing declared extension programs or resources.
- **Resource:** Instruction material or documentation made discoverable to the engine.

### Interfaces

- **Application:** The session operations shared by embedding and frontends. _Avoid_: app.
- **CLI:** The command-line frontend, including chat, print, JSON and RPC modes.

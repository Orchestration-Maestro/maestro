# Glossary

Add terms here when the specs establish their meaning; use one name per concept.

## Models

From `docs/specs/2026-10-04-models.md`.

- **Provider:** A registered adapter that supplies models, implements declared operations and resolves its request authentication.
- **Model identity:** The combination of provider ID, model ID and operation.
- **Operation:** A kind of model work: chat, classification, image generation, embedding or reranking.
- **Catalog:** The published collection of model entries with their identities, capabilities and price metadata.
- **Request:** An invocation of a selected model operation with its inputs, options, authentication and cancellation.
- **Attempt:** One try at fulfilling a request; failed assistant attempts remain in raw history but are omitted from retry projection.
- **Model event:** An ordered update or terminal outcome emitted by a model stream, distinct from its JSON/RPC projection.
- **Scripted fake:** A provider adapter driven by queued responses or request-inspecting factories, with observable calls and no external I/O.

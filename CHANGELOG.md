# Changelog

## [Unreleased]

### Changed

- Narrowed the model-access specification to local catalogs, plain conversation
  projection, flat usage/cost and selected-provider request authentication.
- Clarified model-owned tool-argument validation, derived usage totals and the
  boundary with engine-owned credential lifecycle and model reconciliation.
- Defined chat connection timeout, retry and completion behavior; retained native
  classification, image generation, embeddings and reranking contracts.
- Removed deferred model responses and expanded transcript/catalog lifecycle
  requirements from core scope; required feature documentation in model tickets.

### Added

- Deterministic chat-stream conformance with indexed text/thinking/tool-call
  blocks, strict completed tool JSON, owned metadata snapshots and wakeable
  request cancellation. Expanded the scripted provider with queued factories,
  controlled waits and owned dispatch observations.

- Credential-free model text access with explicit registration, owned streaming
  snapshots, completion and a queued scripted provider adapter.

- Engine-core specification covering application modes, the agent loop, native
  tools, branchable sessions, replaceable storage, settings, extensions, packages
  and documentation acceptance. This specifies planned behavior, not implemented
  runtime features. Clarifies credential precedence, same-identity active-model
  rebinding, unfiltered directory listing and atomic duplicate-registration
  rejection.

- Effective layered settings with origins, manifest value locks and an injectable
  in-memory settings adapter. Reload retains stored state and discards overrides.

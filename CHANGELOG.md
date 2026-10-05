# Changelog

## [Unreleased]

### Changed

- Kept offline workspace graph checks within the test build's fetched crates,
  while retaining declared checks for inactive optional dependencies.
- Enforced scoped crate classes and direct declared/resolved dependency edges,
  with isolated leaves and a separately acyclic restricted test graph.

- Narrowed the model-access specification to local catalogs, plain conversation
  projection, flat usage/cost and selected-provider request authentication.
- Clarified model-owned tool-argument validation, derived usage totals and the
  boundary with engine-owned credential lifecycle and model reconciliation.
- Defined chat connection timeout, retry and completion behavior; retained native
  classification, image generation, embeddings and reranking contracts.
- Removed deferred model responses and expanded transcript/catalog lifecycle
  requirements from core scope; required feature documentation in model tickets.

### Added

- Declared request capabilities with requested/effective thinking, upward-first
  effort clamping, exact shared-ceiling token budgets and simple output defaults.
  Registered option resolution preserves supported preferences and cancellation
  across replaceable adapters; scripted factories observe effective choices.
- Model attempts expose derived flat usage/cost estimates with explicit reported
  and unpriced state, retained on partial failures and cancellation.

- Selected-provider request authentication with explicit secret-free access,
  metadata-only configured status, optional owner-invoked token exchange,
  case-insensitive literal header overlay, safe diagnostics and cancellable
  auth waits. Credential lifecycle remains outside model access.

- Mixed conversation records with nonmutating selected-model projection, opaque
  replay preservation, explicit image omissions and paired missing-result repair.
  Pure offline tool-argument validation returns independent coerced objects
  without tool execution or history writes.

- Deterministic chat-stream conformance with indexed text/thinking/tool-call
  blocks, strict completed tool JSON, owned metadata snapshots and wakeable
  request cancellation. Expanded the scripted provider with queued factories,
  controlled waits and owned dispatch observations.
- Explicit ephemeral session-record storage with atomic batches, detached reads,
  independent handle close/reopen and reusable adapter conformance. Memory is
  non-resumable and provides no restart persistence.

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

- File-backed settings preserving unrelated edits and unknown keys, safe malformed-file
  reload diagnostics, explicit configuration locations and governed session-directory
  selection. File transactions use native cooperative exclusion; in-place writes
  do not promise rollback or crash durability.

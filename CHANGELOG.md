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

- Engine-core specification covering application modes, the agent loop, native
  tools, branchable sessions, replaceable storage, settings, extensions, packages
  and documentation acceptance. This specifies planned behavior, not implemented
  runtime features.

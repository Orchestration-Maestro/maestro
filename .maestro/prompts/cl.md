---
description: Audit release metadata before a release
---
Audit the generated release metadata for every commit since the selected prior release.
Do not edit CHANGELOG.md in implementation PRs, and do not invent historical releases.

## Process

1. **Find the last release tag:**
   ```bash
   git tag --sort=-version:refname | head -1
   ```

   Select an actual prior release tag. If none exists, report the missing tag
   and ask for an explicit comparison base before you audit.

2. **List every commit since that tag:**
   ```bash
   git log <tag>..HEAD --oneline
   ```

3. **Read the release metadata of each affected delivered crate:**
   Inspect conventional commits, PR descriptions and available generated notes.
   Do not assume an unreleased section or a release command exists.

4. **Check each commit:**
   - Skip: changelog updates, doc-only changes and release housekeeping.
   - Skip: changes to generated model catalogs (for example `crates/maestro-models-catalog/src/`), unless an intentional product-facing change in non-generated source or docs comes with them.
   - Find the package or packages the commit affects (use `git show <hash> --stat`).
   - Verify that the release metadata covers each affected crate.
   - For an external contribution (PR), verify the format: `Description ([#N](url) by [@user](url))`

5. **Cross-package duplication rule:**
   Repeat a lower-layer change in the user-facing release summary when end users
   feel it. Never present an internal-only change as an end-user feature.

6. **Propose New Features after you audit the missing entries:**
   - Propose the top new features under `New Features` in the release metadata.
   - Get the user's confirmation before you write them.
   - Link the relevant docs and sections wherever possible.

7. **Report:**
   - List the commits with missing entries.
   - List the entries that need cross-package duplication.
   - Fix omissions through a conventional commit, a PR or release metadata.
     Never use manual changelog edits or an invented publishing command.

## Release Metadata Classification

Classify each entry as one of:
- `### Breaking Changes` - API changes that require migration
- `### Added` - New features
- `### Changed` - Changes to existing functionality
- `### Fixed` - Bug fixes
- `### Removed` - Removed features

Attribution examples (illustrative, not release history):
- Internal: `Fixed foo ([#123](https://github.com/Orchestration-Maestro/maestro/issues/123))`
- External: `Added bar ([#456](https://github.com/Orchestration-Maestro/maestro/pull/456) by [@user](https://github.com/user))`

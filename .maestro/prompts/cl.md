---
description: Audit changelog entries before release
---
Audit generated-release metadata for all commits since the selected prior release.
Do not edit CHANGELOG.md in implementation PRs or invent historical releases.

## Process

1. **Find the last release tag:**
   ```bash
   git tag --sort=-version:refname | head -1
   ```

   Select an actual prior release tag. If none exists, report the missing tag
   and request an explicit comparison base before auditing.

2. **List all commits since that tag:**
   ```bash
   git log <tag>..HEAD --oneline
   ```

3. **Read release metadata for every affected delivered crate:**
   Inspect conventional commits, PR descriptions and available generated notes.
   Do not assume an unreleased section or a release command exists.

4. **For each commit, check:**
   - Skip: changelog updates, doc-only changes, release housekeeping
   - Skip: changes to generated model catalogs (for example `crates/maestro-models/src/catalog/models_generated/`) unless accompanied by an intentional product-facing change in non-generated source/docs.
   - Determine which package(s) the commit affects (use `git show <hash> --stat`)
   - Verify release metadata covers the affected crate(s)
   - For external contributions (PRs), verify format: `Description ([#N](url) by [@user](url))`

5. **Cross-package duplication rule:**
   Lower-layer changes that affect end users also belong in the user-facing
   release summary. Internal-only changes do not become end-user features.

6. **Propose New Features after auditing missing entries:**
   - Propose the top new features under `New Features` in release metadata.
   - Obtain the user's confirmation before writing them.
   - Link to relevant docs and sections whenever possible.

7. **Report:**
   - List commits with missing entries.
   - List entries that need cross-package duplication.
   - Remedy omissions through conventional commit, PR or release metadata,
     not manual changelog edits or an invented publishing command.

## Release Metadata Classification

Classify entries using:
- `### Breaking Changes` - API changes requiring migration
- `### Added` - New features
- `### Changed` - Changes to existing functionality
- `### Fixed` - Bug fixes
- `### Removed` - Removed features

Attribution examples (illustrative, not release history):
- Internal: `Fixed foo ([#123](https://github.com/Orchestration-Maestro/maestro/issues/123))`
- External: `Added bar ([#456](https://github.com/Orchestration-Maestro/maestro/pull/456) by [@user](https://github.com/user))`

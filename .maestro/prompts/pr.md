---
description: Review PRs from URLs with structured issue and code analysis
argument-hint: "<PR-URL>"
---
You are given one or more GitHub PR URLs: $@

For each PR URL, do the following in order:
1. Add the `in-progress` label to the PR via GitHub CLI before analysis starts. If adding the label fails, report that explicitly and continue.
2. Read the PR page in full. Include description, all comments, all commits, and all changed files.
3. Identify any linked issues referenced in the PR body, comments, commit messages, or cross links. Read each issue in full, including all comments.
4. Analyze the PR diff. Read all relevant code files in full with no truncation from the current main branch and compare against the diff. Do not fetch PR file blobs unless a file is missing on main or the diff context is insufficient. Include related code paths that are not in the diff but are required to validate behavior.
5. Check generated-release metadata for each affected crate: conventional commits, PR descriptions and available release notes. Report missing entries through that metadata; do not edit CHANGELOG.md in implementation PRs. Follow the release rules in AGENTS.md. Verify:
   - Entries use the correct classification (`Breaking Changes`, `Added`, `Changed`, `Fixed`, `Removed`).
   - External contributions include PR link and author: `Fixed foo ([#123](https://github.com/Orchestration-Maestro/maestro/pull/123) by [@user](https://github.com/user))`
   - Breaking changes are classified as `Breaking Changes`, not just `Fixed`.
6. Check whether README.md, docs/ and examples require modification. This is usually the case when existing features have been changed, or new features have been added.
7. Provide a structured review with these sections:
   - Good: solid choices or improvements
   - Bad: concrete issues, regressions, missing tests, or risks
   - Ugly: subtle or high impact problems
8. Add Questions or Assumptions if anything is unclear.
9. Add Change summary and Tests.

Output format per PR:
PR: <url>
Changelog:
- Assess generated-release metadata, not manual changelog edits.
- ...
Good:
- ...
Bad:
- ...
Ugly:
- ...
Questions or Assumptions:
- ...
Change summary:
- ...
Tests:
- ...

If no issues are found, say so under Bad and Ugly.

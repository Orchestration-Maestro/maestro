---
description: Review pull requests by URL with linked-issue and code analysis
argument-hint: "<PR-URL>"
---
Review the GitHub PRs at these URLs: $@

For each PR URL, work in this order:
1. Before analysis starts, add the `in-progress` label to the PR with the GitHub CLI. If adding the label fails, report the failure and continue.
2. Read the whole PR page: the description, every comment, every commit and every changed file.
3. Find the issues the PR links in its body, comments, commit messages or cross-references. Read each issue in full, including every comment.
4. Analyze the PR diff. Read every relevant code file in full, without truncation, from the current main branch, and compare it with the diff. Fetch PR file blobs only when a file is missing on main or the diff context is not enough. Include the related code paths outside the diff that you need to validate behavior.
5. Check the generated release metadata of each affected crate: conventional commits, PR descriptions and available release notes. Report missing entries through that metadata; do not edit CHANGELOG.md in implementation PRs. Follow the release rules in AGENTS.md. Verify:
   - Each entry uses the correct classification (`Breaking Changes`, `Added`, `Changed`, `Fixed`, `Removed`).
   - An external contribution carries its PR link and author: `Fixed foo ([#123](https://github.com/Orchestration-Maestro/maestro/pull/123) by [@user](https://github.com/user))`
   - A breaking change is classified as `Breaking Changes`, never only as `Fixed`.
6. Check whether README.md, docs/ and examples need changes. They usually do when an existing feature changes or a new one is added.
7. Write a structured review with these sections:
   - Good: solid choices or improvements
   - Bad: concrete issues, regressions, missing tests or risks
   - Ugly: subtle or high-impact problems
8. Add Questions or Assumptions for anything unclear.
9. Add a Change summary and Tests.

Output format per PR:
PR: <url>
Changelog:
- Assess the generated release metadata, never manual changelog edits.
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

If you find no issues, say so under Bad and Ugly.

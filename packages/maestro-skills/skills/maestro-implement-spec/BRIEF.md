# Implementer brief

Write one file per bounded task. Link the authoritative task and spec instead of
copying their entire text. Use the [subagent contract](../../references/subagents.md)
and [report statuses](../../references/report.md).

## Required fields

- Task: tracker link, goal and exact scope.
- Context: spec, relevant sources, existing tests, earlier decisions and reports.
- Retained interfaces: inputs, outputs, ownership, errors and behaviour to preserve.
- Constraints: repository rules, excluded work, allowed edits and dependencies.
- Working directory: actual isolated worktree, branch and integration base.
- Report location: persistent file for the initial report and appended fix reports.
- Acceptance: independently checkable observations and exact verification selectors.

## Instructions to the implementer

Read the referenced contract before editing. Ask about unclear requirements before
choosing an interpretation. Check that your worktree derives from the integration
branch and that no other worker owns it. Implement only the named task.

Use `/skill:maestro-tdd` vertically at the agreed public seams: one observation,
its first result, then the smallest implementation. Iterate with focused tests.
Preserve existing assertions and error behaviour. Report environment failures instead
of bypassing them. Merge the integration branch's latest tip before reporting and
verify the composed work.

Self-review the diff for completeness, accidental scope, unnecessary code, naming,
errors and documentation. Apply the [completion checks](../../references/definition-of-done.md).
Never dispatch children; report missing context or a need for another investigation
to the controller. Append fix reports to the same report file so a replacement can
see the attempts and their evidence.

Return exactly one status: `DONE`, `DONE_WITH_CONCERNS`, `NEEDS_CONTEXT` or `BLOCKED`.
Include commits, changed files, commands and observed test results, concerns, unresolved
questions and what remains. A claim without completed verification is not `DONE`.

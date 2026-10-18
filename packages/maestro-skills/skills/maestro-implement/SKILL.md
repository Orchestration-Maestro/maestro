---
name: maestro-implement
description: "Use when an approved ticket, specification or agreed change is ready to build."
disable-model-invocation: true
---

# Implement the agreed change

Fetch the ticket and comments through the [tracker](../../references/issue-tracker.md), and state its title.
Resolve an ambiguous reference before starting. Read the linked spec, relevant source and agreed test seams.
Before non-trivial work, write numbered assumptions the user can correct.

## Preserve the run record

Use `.scratch/<plan>/progress.md` as a run ledger, with the plan's identity first. Read it and `git log` before
resuming; completed work is not redone because conversation memory is missing. `todo` is a live view, not the record.
Check that interfaces one ticket produces agree with those later tickets consume. Record rulings beside the conflict.
Do not silently change a requirement: if instructions conflict, stop, explain the confusion and trade-off, and wait.

Stop for an irreversible or destructive operation, a security-sensitive action, a side effect outside the
worktree that requires permission, or a plan so broken that every path forward is a guess. Ask before proceeding.
For every stop say what is done, what remains, the evidence and what unblocks it. Use the
[completion statuses](../../references/report.md), not an implied success.

## Build one slice

Use `/skill:maestro-tdd` at agreed seams. Prefer a vertical path; use contract-first slices when callers and
providers develop separately and risk-first slices when one uncertainty could invalidate the work.
Keep one logical change in progress. Keep it compiling, use safe defaults for unfinished work and prefer
independently reversible changes. Do not mix adjacent cleanup into the feature.

Run type checks and focused tests regularly as code changes. Run the repository's full suite once at the end.
Compare each expected result with actual output; a plan step claiming success is not evidence.
If the plan is wrong, reconcile it against its spec and record the ruling before continuing.

## Close the slice

Use `/skill:maestro-code-review` before committing. Resolve justified findings, then use `/skill:maestro-verify`
for completion evidence. Commit with the repository's normal conventions and update the ledger with the
commit range, actual verification and status. Carry completed work forward into the next slice.

Use `/skill:maestro-git-workflow` for commit handling and isolation, retaining the TDD, review
and completion checks above.

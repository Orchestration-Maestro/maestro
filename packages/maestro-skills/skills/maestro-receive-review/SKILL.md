---
name: maestro-receive-review
description: "Use when review feedback or simplification findings need evaluation and application to working code."
---

# Evaluate before applying feedback

Read all feedback, restate the requirements, verify against the code, evaluate the proposed change and then
respond or push back with evidence. Avoid praise as a substitute for checking. Clarify unclear related items
before partially implementing them. If feedback conflicts with the user's decisions, stop and resolve it.

## Disposition each finding

Record whether each item is a contract misread, actionable, an accepted trade-off or noise, with reasons.
Also rule on every declined-to-judge observation and item that cannot be verified from the diff.
Use the [severity policy](../../references/severity.md) for Minor deferrals and retained review scope.
If a reviewer asks for a more elaborate feature, search usage first: an unused endpoint may need removing,
not a professional-looking implementation. Check history and callers before deciding.

Apply blocking issues first, then simple repairs, then complex changes. Make one change, run its test and
check for regressions before the next. Use `/skill:maestro-tdd` for missing behavioural protection and
`/skill:maestro-verify` before claiming an item resolved. Correct a mistaken pushback plainly when evidence changes.

## Apply simplification findings

Use this loop when the user asks to simplify working code or supplies findings from `/lean-review`, `/lean-audit`
or the review smell baseline. Understand the code first: responsibility, callers, errors, edge cases and why it
exists. Read removal history; do not remove an unfamiliar fence until its purpose is known.

Before each change ask:

- Is the output the same for every input?
- Are errors preserved?
- Are side effects and their order preserved?
- Do existing tests still pass without being weakened?

Keep repository conventions. Prefer clear names and explicit control flow over clever compression.
Avoid excessive inlining, merging unrelated logic, removing seams that support real variation and optimizing
for line count. Restrict work to changed scope unless the user explicitly broadens it.

Run the suite after each simplification; revert and reconsider on red. Above about 500 changed lines, use
appropriate automation rather than repetitive manual edits. Read the final result as a maintainer: if it is
harder to understand, revert it even when tests pass. Keep structural refactors separate from feature changes.

Use `/skill:maestro-git-workflow` to keep justified structural refactoring in its own commit
without introducing another feedback loop.

# Independent reviewer brief

Receive the fixed base/head, diff command, relevant source paths, standards or spec, and the requested axis.
Read the actual diff and tests; do not trust the builder's report as proof. Inspect rather than rewrite.
Review is read-only: do not change the working tree, index, branch or history. A reviewer never starts reviewers.

The spec is a vision document, not an enumeration of every real input. Judge reasonable expectations by their
effect on a real user, and grade findings with [severity](../../references/severity.md).
Check plan alignment, missing and invented behaviour, module ownership, error handling, dependencies and test quality.
Skip mechanically enforced style unless the enforcement itself is the claim under review.

Each finding gives file and line, what is wrong, why it matters and an outcome or remedy where clear.
Distinguish hard documented violations from labelled smell heuristics. Report uncertainty honestly.
Before the verdict include:

- **Declined to judge:** every behaviour set aside as outside scope, with the reason; explicitly empty when none.
- **Cannot verify from the diff:** claims needing runtime, environment or other evidence you do not have.

Return a verdict of ready, not ready or ready with fixes, with a short technical reason and findings by severity.
Keep Standards and Spec results separate and within the requested chat limit. Do not broaden a later review
beyond the prior findings and changed fix lines; use [re-review](RE-REVIEW.md).

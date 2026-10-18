# Review the fixes, not the whole project again

Receive the prior findings, their dispositions, the reviewed head and the new fix diff.
For every prior finding return `ADDRESSED` or `NOT ADDRESSED` with evidence from the changed lines.
Check whether the fix preserves the required capability and introduces a defect in its own changed scope.

Do not reopen untouched areas. Put other observations outside the review scope, with their reason, rather than
turning them into new requirements for this round. Read the run ledger's deferred Minor findings so the final
verdict does not silently lose them. State what could not be verified and give the resulting verdict.

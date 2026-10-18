# Completion contract

Use one status that matches the evidence:

- `DONE`: the task and all required verification are complete.
- `DONE_WITH_CONCERNS`: the requested work is complete but named concerns remain for the caller.
- `NEEDS_CONTEXT`: a specific missing fact or decision prevents a reliable implementation.
- `BLOCKED`: a prerequisite or obstacle prevents progress.

Report what changed, the files and commit range, exact commands with their observed results, the acceptance
criteria checked, and remaining concerns. For incomplete work, say what is finished, what remains and what
would unblock it. Do not substitute a positive tone for a missing test or silently change the contract.
A parent compares this report with the actual diff and evidence before accepting completion.

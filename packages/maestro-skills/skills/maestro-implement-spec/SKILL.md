---
name: maestro-implement-spec
description: "Use when an approved specification has dependent tickets to deliver together."
disable-model-invocation: true
---

# Deliver the whole task graph

Read the spec and all tickets through the [tracker](../../references/issue-tracker.md).
Tickets form a blocking graph, not a numbered script. Inspect the existing run ledger,
completed work, open findings and working trees before starting another run. Reconcile
conflicting plans against the approved spec; record a ruling before changing direction.

Use [subagent briefs and model selection](../../references/subagents.md), the existing
[completion statuses](../../references/report.md) and [definition of done](../../references/definition-of-done.md).
Keep communication sparse: pass source pointers, ticket links and report paths rather
than duplicating the spec. Maintain a visible ledger of ticket heads, ownership,
blocking edges, review findings, deferred work and rulings. Include the consequence
and cost if a ruling is wrong, so the user can challenge it.

## Preflight and ready frontier

1. Establish the integration branch and its baseline. Use `/skill:maestro-git-workflow`
   to detect existing isolation and permission requirements before creating worktrees.
   Check for dirty files, live owners and conflicting work. Repair environment faults
   before considering a workaround; report a failing baseline rather than hiding it.
2. Find the open, unblocked, unclaimed tickets. Dispatch independent problem domains
   in parallel, each to one background implementer with its own branch and worktree
   based on the integration branch. Dependent tickets wait for their prerequisites.
3. Write each implementer's [BRIEF](BRIEF.md), naming its actual directory, retained
   interfaces, acceptance and persistent report path. The implementer reads the task,
   asks before guessing and uses `/skill:maestro-tdd` at the agreed seams.
4. Before reporting, each implementer merges the latest integration tip into its
   branch and verifies the composed result. Do not reset away someone else's work.
5. Use a merger subagent to integrate completed, reviewed work. When requests are
   the repository's delivery mechanism, open a draft after the first merge creates
   a real diff, linking the spec and tickets. Recompute and refill the ready frontier
   after every merge; a worker finishing is not proof its dependents can start.

## Handle every report

- `DONE`: inspect its commits and covering verification before admitting it to review.
- `DONE_WITH_CONCERNS`: inspect the work and classify each concern; a blocking concern
  keeps the ticket open even when the implementation is complete.
- `NEEDS_CONTEXT`: supply the missing source or settle the decision before resuming.
- `BLOCKED`: repair the prerequisite, environment or ownership conflict, or park the
  ticket visibly and continue only independent work.

Stop for destructive or irreversible work, security-sensitive actions, external side
effects requiring permission, or a plan with no reliable path forward. Ask the user
rather than inventing authority. The controller coordinates; it never fixes findings.

## Review each ticket

Every ticket receives `/skill:maestro-code-review` on both Standards and Spec axes.
Give reviewers the fixed diff and source contract, not the builder's preferred answer.
Resolve every cannot-verify observation using the cross-ticket context. A confirmed gap
enters the fix loop. Classify findings through the [severity policy](../../references/severity.md).
Record Minor findings as deferred and give the final reviewer that list.

A finding that conflicts with a required plan choice needs a recorded ruling against
that plan and the spec. Neither dismiss it because the plan says so nor silently fix
it by contradicting the plan. Distinguish load-bearing gaps, which prevent safe or
correct delivery, from parkable improvements with a visible disposition.

## Fix loop

A round is one fix dispatch plus one scoped re-review. Allow at most five per ticket.
Rounds 1-3 resume the implementer where the harness supports it; otherwise send a
fresh implementer the original brief, persistent report and verbatim open findings.
Rounds 4-5 use a fresh implementer on a more capable model. Explain what was tried
and transfer ownership rather than asking for another blind patch.

Every fixer appends its report with changed commits, named covering tests, exact
commands and actual output. Inspect that evidence before re-review. The reviewer
checks the prior findings and changed lines, marks each addressed or not addressed,
and flags new breakage in the fix. Unrelated discoveries become separate observations,
not an expanding audit. At the fifth failed round, stop and surface the unresolved
cause, attempted approaches and decisions needed; do not start round six.

## Whole-branch completion

When the graph is complete, use the most capable available model for a whole-branch
review. Supply the spec, integration diff, rulings, deferred Minor findings and parked
items. Triage them explicitly rather than letting a roll-up disappear. Dispatch one
combined fix wave to an implementer, followed by one scoped re-review. The controller
does not make the repairs. Stop for a new blocking gap rather than hiding it in a
completion claim.

Use `/skill:maestro-verify` on the integrated branch and report actual acceptance
results. Mark the draft ready or resolve the tickets according to repository policy.
Clean up only worktrees owned by this run after preserving their commits and reports;
leave retained requests and externally managed workspaces intact.

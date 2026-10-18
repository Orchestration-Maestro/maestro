---
name: maestro-code-review
description: "Use when a branch, pull request or changed code needs independent standards and specification review."
---

# Review a fixed diff on two axes

Resolve the supplied fixed point with `git rev-parse`, record `git diff <fixed-point>...HEAD` and the commit list,
and reject an empty diff. Ask for a fixed point when none is supplied.
Find the originating spec from issue references and the [tracker](../../references/issue-tracker.md), then an
explicit user path, then a matching repository spec. If none exists, ask; report “no spec available” when appropriate.

## Standards sources and smell baseline

Read the repository's rules, contribution guide, coding standards and domain vocabulary.
Repository standards override these heuristics, which are judgement calls rather than automatic violations:

| Smell | Possible remedy |
| --- | --- |
| Mysterious Name | Rename to expose the responsibility; investigate unclear design. |
| Duplicated Code | Give one shared shape one owner and call it. |
| Feature Envy | Move behaviour to the owner of the data it uses. |
| Data Clumps | Give repeatedly travelling fields one named record. |
| Primitive Obsession | Model a real domain concept instead of an unstructured primitive. |
| Repeated Switches | Give repeated dispatch one map or polymorphic owner. |
| Shotgun Surgery | Gather one change's scattered knowledge. |
| Divergent Change | Separate responsibilities that change for unrelated reasons. |
| Speculative Generality | Delete unused flexibility and hypothetical hooks. |
| Message Chains | Hide a navigation walk behind its owner. |
| Middle Man | Remove delegation that supplies no useful interface. |
| Refused Bequest | Prefer composition when an implementation rejects its inherited contract. |

## Independent passes

Read tests before implementation. Compare what they prove with the contract; do not accept assertion count as
behaviour coverage. Run one inverted-condition experiment where feasible: if tests remain green after a meaningful
condition is reversed, identify the missing behavioural test. Be explicit about experiments you could not run.

Dispatch two fresh parallel `subagent` reviews using [REVIEWER](REVIEWER.md) and the
[subagent brief](../../references/subagents.md). Both receive the same fixed diff and commit list.
Standards receives its source list and the complete smell baseline. Spec receives the actual originating contract.
Each axis reports at most 400 words in chat. Present **Standards** and **Spec** separately; do not merge or rerank them.
A standards pass cannot excuse the wrong feature, and a spec pass cannot excuse poor structure.

Prefer structural remedies to another patch. Check dependency direction, canonical reuse and unnecessary public
surface. Ask why something was removed; unexplained removal is a finding, not an assumed cleanup.
Report what cannot be verified from the diff rather than claiming a run you did not perform.

Grade by [severity](../../references/severity.md). Subsequent reviews use [RE-REVIEW](RE-REVIEW.md) and the fix diff,
not an expanding full audit. End with findings per axis and the worst severity within each axis, without a single winner.

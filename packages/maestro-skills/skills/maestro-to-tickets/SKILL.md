---
name: maestro-to-tickets
description: "Use when an approved specification or plan needs independently reviewable implementation tickets."
disable-model-invocation: true
---

# Break the work into vertical slices

Read the whole source spec or issue and its comments through the [tracker](../../references/issue-tracker.md).
Use domain names and relevant ADRs. Inspect the code where needed to understand the current seams and dependencies.

## Plan dependencies

Each tracer bullet delivers a complete, observable path through the affected layers, not one horizontal layer.
Prefactor first when needed: make the change easy, then make the easy change.
Draw a dependency graph and mark verification checkpoints. Distinguish independent work from true blockers.
Never overwrite an unfinished plan; reconcile its completed work and unresolved decisions first.

For a wide mechanical refactor that cannot stay green in vertical slices, use expand–migrate–contract:
add the new form, move callers in independently verifiable batches, then remove the old form after all migrations.
If even batches cannot stand alone, name one integration branch and the final integrate-and-verify ticket.

## Each ticket carries

- The user-visible result and acceptance checks, with the originating spec link.
- Source files and existing tests needed to locate and verify the change, rather than implementation code.
- Interfaces consumed and produced, global constraints and explicit failure modes for review.
- Blocking edges to real prerequisites, a verification checkpoint and the applicable
  [definition of done](../../references/definition-of-done.md).
- A self-review of completeness, scope and proportion: does the work earn its complexity?

Split at a review boundary: a reviewer could reject one ticket and approve its neighbour.
Line counts and file counts do not define a capability or a useful split. Keep each ticket self-contained
so a fresh worker can use it without the entire planning conversation.

## Approval and publication

Show a numbered proposal with titles, blocking edges and delivered outcomes. Ask whether the boundaries and
edges are right, and whether tickets should merge or split. Stop for approval before implementation.
Publish approved tickets in dependency order, linking children to their parent and using native blocking edges
where available. Local tracking uses one numbered file per ticket, never one combined list.
Work the unblocked frontier. Do not close or rewrite the parent merely because its tickets were created.

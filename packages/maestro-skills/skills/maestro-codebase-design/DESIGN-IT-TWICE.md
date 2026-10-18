# Compare independently designed interfaces

Explain the problem space to the user: constraints, dependency categories and a small illustrative usage sketch.
The sketch grounds the problem; it is not the selected solution.

Run independent parallel `subagent` designs with [self-contained briefs](../../references/subagents.md).
Give at least three distinct constraints: minimize the interface, maximize justified flexibility, and make the
common caller trivial. A fourth can concentrate on ports and adapters where dependencies cross a seam.
Pass domain vocabulary and the same underlying constraints, but do not let one design seed the others.

Each result gives the interface, invariants, ordering, error modes, example use, hidden implementation,
dependency strategy and trade-offs. Compare them by depth, locality and seam placement, not source-line count.
Present the alternatives, recommend one with reasons and propose a hybrid only when its combined complexity is justified.

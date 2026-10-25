---
name: maestro-codebase-design
description: "Use when a module interface, test seam, deepening opportunity or adapter placement needs design."
---

# Design deep modules

## Vocabulary

- **Module:** something with an interface and an implementation, from a function to a tier-spanning slice.
- **Interface:** everything a caller must know: types, invariants, ordering, errors, configuration and cost.
- **Implementation:** the code hidden behind that interface.
- **Depth:** behaviour callers can exercise per unit of interface they must learn.
- **Seam:** the place where behaviour can vary without editing the consumer.
- **Adapter:** a concrete implementation satisfying an interface at a seam.
- **Leverage:** what callers gain from depth; one implementation serves many callers.
- **Locality:** change and knowledge concentrated in one place rather than scattered among callers.

Use these terms consistently. Depth is not a ratio of source lines. A small interface hiding useful behaviour
is deep; an interface nearly as complex as its implementation is shallow.

## Test the shape

Ask whether fewer entry points or simpler parameters can hide more complexity without losing capability.
Apply the deletion test: if deleting a module makes complexity disappear, it was passing work through;
if complexity reappears across its callers, it was earning its place.

The interface is the test surface. If a test needs to reach past it, reconsider the shape of the module.
An internal seam need not become public because a test uses it. One adapter is a hypothetical variation;
two justified adapters establish a real seam. Do not add an abstraction for an imagined future consumer.

Accept dependencies rather than constructing them inside logic, and return results where effects are unnecessary.
Reduce complexity rather than move it into another file. Search for and reuse the canonical helper instead of
adding a nearly identical implementation. Follow the repository's dependency direction.

## Explore alternatives

Use [deepening](DEEPENING.md) to classify dependencies and replace shallow tests at the new seam.
Use [design it twice](DESIGN-IT-TWICE.md) for independent parallel `subagent` designs with distinct constraints.
Compare depth, locality and seam placement, then recommend a design with its trade-offs.
Use [contracts](CONTRACTS.md) for public-surface commitments, validation and error policy.

---
name: maestro-to-spec
description: "Use when an agreed conversation or design needs a written specification."
disable-model-invocation: true
---

# Write the specification

Synthesize what is already agreed; do not restart the interview. Read the repository's domain glossary and
relevant ADRs, using `/skill:maestro-domain-modeling` and `/skill:maestro-codebase-design` when their vocabulary is needed.
List assumptions explicitly. Turn vague goals into checkable success criteria rather than inventing targets.

## Establish scope and seams

When a request contains several independently testable capabilities, propose a capability map first:
stable capability names, responsibilities, dependencies and build order. The user approves it before individual specs.
A single capability does not need this extra map.

Prefer existing test seams, at the highest useful public interface and with as few seams as possible.
Describe what each catches and misses; agree them with the user before writing tests.

## Template

- **Problem:** the user's problem and why it matters.
- **Solution:** the outcome from the user's perspective.
- **User stories:** numbered actor, capability and benefit statements covering the feature.
- **Implementation decisions:** agreed module/interface changes, contracts, interactions and technical constraints.
- **Testing decisions:** agreed seams, observable behaviours, prior art and exact repository verification commands.
- **Assumptions and success criteria:** unresolved beliefs and conditions that can be checked.
- **Boundaries:** always do, ask first and never do.
- **Out of scope:** exclusions and adjacent work this does not deliver.

Avoid brittle file lists and implementation code in a vision document. A decision-rich prototype fragment may
stay when it expresses an agreed contract more precisely than prose; identify it as such.

Publish through the [issue tracker](../../references/issue-tracker.md). Where the repository keeps specs in
`docs/specs/`, `SPEC.md` or another chosen home, follow that convention instead of making a duplicate system.
Keep the spec alive: update changed decisions and scope before implementing them and link it from later work.

## Builder check and approval

Use `/skill:maestro-verify` for a fresh builder reading of the written design. Give the builder the artifact and
contract, not your conclusions. Sort questions into answered by the spec, minor clarifications you can resolve,
and decisions belonging to the user. Resolve the first two; put the remaining decisions to the user in one round.

Summarize the spec and open questions, ask for approval and stop. Do not start tickets or code until approval.

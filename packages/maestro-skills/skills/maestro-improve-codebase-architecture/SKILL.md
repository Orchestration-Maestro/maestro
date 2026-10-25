---
name: maestro-improve-codebase-architecture
description: "Use when architectural friction or shallow modules need investigation before choosing a refactor."
disable-model-invocation: true
---

# Find opportunities to deepen a module

Use `/skill:maestro-codebase-design` for module, interface, depth, seam, adapter,
leverage and locality. Read the domain glossary and relevant ADRs through
`/skill:maestro-domain-modeling`. Preserve established names rather than inventing
synonyms for each candidate.

## Explore a justified scope

Take a supplied direction. Otherwise use commit history to find recently changing
hot spots; widen the search when no area stands out. Give a fresh investigator the
[shared subagent brief](../../references/subagents.md), sources and scope.

Look for one concept scattered across modules, an interface nearly as complex as its
implementation, hidden coupling across a seam and tests that exercise extracted pure
functions while missing the actual caller. Apply the deletion test: would deleting
the suspect module concentrate complexity or merely move it? Prefer real friction
over a mechanical checklist of fashionable refactors.

## Present before designing

Use [REPORT](REPORT.md) for a temporary offline page. Each candidate names files,
observed friction, proposed deepening, benefits in locality/leverage, before/after
structure and recommendation strength. Surface an ADR conflict only when concrete
friction justifies revisiting it. Do not propose detailed interfaces before the user
chooses a candidate.

Ask which candidate to explore. Use `/skill:maestro-grilling` on the chosen one to
settle constraints, what sits behind the seam and which tests survive. Update settled
glossary terms as they emerge. Offer an ADR only for a durable, non-obvious reason
future explorers need, not an ephemeral 'not now'. For alternative interfaces use the
existing design-it-twice method in codebase-design instead of inventing another loop.

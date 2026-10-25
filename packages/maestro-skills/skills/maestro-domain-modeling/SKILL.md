---
name: maestro-domain-modeling
description: "Use when domain terms conflict, concepts are fuzzy, a glossary is changing, or a durable design decision needs recording."
---

# Sharpen the domain model

Read the repository's glossary and relevant ADRs. This is an active discipline, not only a vocabulary lookup:
challenge conflicting terms and ambiguous relationships while designing.

## Challenge with evidence

If a term conflicts with its definition, show both meanings and ask which is intended.
For an overloaded term, propose one canonical name. Probe relationships with concrete edge cases:
can an order be partly cancelled, and is a customer the same entity as a signed-in user?
Check the code when a claim about current behaviour matters. Surface contradictions instead of hiding them.

## Record terms as they settle

Update the existing glossary inline, not at the end of the session. Follow its current name and format;
use `CONTEXT.md` when there is none. [Glossary format](GLOSSARY-FORMAT.md) describes canonical terms and relationships.
Keep implementation decisions and progress notes out of it.

Most repositories need one glossary. Use a context map only when there are several actual domain contexts.
If a map already exists, follow its links and ask which context applies when unclear. Create files only when
there is something to record.

## Record decisions sparingly

Offer an ADR only when all three are true: it is hard to reverse, surprising without context and the result
of a real trade-off. Otherwise keep the explanation where the work already lives.
Follow the repository's existing ADR convention first; absent one, use `docs/adr/` and the
[ADR format](ADR-FORMAT.md). Preserve an old decision and supersede it when the decision changes; do not delete history.

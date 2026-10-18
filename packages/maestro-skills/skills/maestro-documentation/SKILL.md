---
name: maestro-documentation
description: "Use when README content, API documentation, comments or technical guides need writing or review."
---

# Document what the reader needs

Check technical claims against the owning code, commands and contracts before editing
style. Preserve limits, uncertainty and exceptions that the evidence requires. Choose
the audience and the question the document should answer; remove paragraphs no reader
needs for that purpose.

## Keep knowledge beside its owner

Comments explain why a non-obvious choice exists, including gotchas a refactor might
otherwise erase. Let names and code show routine control flow. Put API contracts near
their public items: inputs, outputs, errors, ownership, side effects and examples.

Keep quick-start commands runnable from their stated directory with prerequisites
and expected results. Check examples against the actual version and configuration.
Use the repository's documentation structure instead of making parallel guides.
Follow its changelog and release tooling; do not hand-edit generated release history.

## Pick the right lens

Agent-facing skills and rules files use `/skill:maestro-writing-for-agents` for discovery,
load and completion criteria. Durable decisions and ADRs use
`/skill:maestro-domain-modeling`; record the reason and trade-off, not routine changes.
Run `/skill:maestro-humanizer` on every changed document for the prose pass. This
refines wording after technical review; it never authorizes adding a fact or removing
a needed qualifier. Keep code, structured data and link destinations intact.

Before finishing, follow local links, run documented commands where safe, check the
examples and delete stale instructions. Leave a specific verification gap visible
when access or prerequisites prevent a check.

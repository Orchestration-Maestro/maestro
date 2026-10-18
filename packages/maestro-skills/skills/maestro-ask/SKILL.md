---
name: maestro-ask
description: "Use when you need to choose a development skill or decide what to do next."
disable-model-invocation: true
---

# Choose the next skill

Read the selected skill's `SKILL.md` before describing it or recommending that a step be skipped.
These summaries orient you; they do not replace the instructions.

## Main flow

1. `/skill:maestro-grilling` establishes the user's intent and resolves design decisions.
2. `/skill:maestro-to-spec` records the agreed outcome, boundaries and test seams. Stop for approval.
3. `/skill:maestro-to-tickets` creates independently reviewable vertical slices and blocking edges. Stop for approval.
4. `/skill:maestro-implement` works a ticket. Build with `/skill:maestro-tdd`, review with
   `/skill:maestro-code-review` and establish evidence with `/skill:maestro-verify`.
5. `/skill:maestro-pr` prepares the pull request; `/skill:maestro-retro` improves the working environment afterward.

For a small, already agreed change, implementation can start without manufacturing a spec and ticket sequence.
For a failure or regression, enter through `/skill:maestro-diagnose` before choosing a fix.

## Vocabulary and feedback

- `/skill:maestro-domain-modeling` challenges terms, records the domain glossary and captures significant decisions.
- `/skill:maestro-codebase-design` supplies module, interface, depth, seam and adapter vocabulary.
- `/skill:maestro-receive-review` evaluates review feedback and applies justified changes one at a time.

Process skills precede build skills: clarify before planning, diagnose before fixing, agree seams before testing.
The user's explicit instructions and the repository's `AGENTS.md` take precedence over these defaults.

## Modes and neighbouring commands

`/lean` controls standing simplification guidance; `/focus` controls standing presentation guidance.
For simplification findings use `/lean-review` or `/lean-audit`, then use receiving review to evaluate and apply them.
A full security audit belongs to `security-audit`, not to a general code review.

At a phase boundary, read [phase boundaries](../../references/phase-boundaries.md) before discarding context.

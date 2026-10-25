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

## Domain work

- `/skill:maestro-security` guides writing secure code; full audits remain with `security-audit`.
- `/skill:maestro-performance` measures bottlenecks and keeps only demonstrated improvements.
- `/skill:maestro-frontend-ui` covers components, state, accessibility and browser verification.
- `/skill:maestro-observability` builds signals operators can use to diagnose production behaviour.
- `/skill:maestro-ship` prepares rollout, monitoring, rollback and retirement.
- `/skill:maestro-quality-gates` records the project's bar and executable checks.

## Other entry points

These are suggestions for the human, not automatic invocations of type-only skills.

- `/skill:maestro-triage` classifies incoming issues and explicitly enabled requests.
- `/skill:maestro-wayfinder` maps a large, foggy effort into decision tickets before specs.
- `/skill:maestro-implement-spec` is the whole-spec alternative to single-ticket implementation.
- `/skill:maestro-prototype` answers an unsettled runnable design question after grilling.
- `/skill:maestro-handoff` carries that exploration or another phase to a new context.
- `/skill:maestro-improve-codebase-architecture` investigates architectural health and shallow modules.
- `/skill:maestro-wait-what` re-explains the last message when context or terminology is missing.
- `/skill:maestro-research` delegates primary-source facts and versioned documentation questions.
- `/skill:maestro-git-workflow` handles isolation, commits, history and branch completion.
- `/skill:maestro-show-options` makes visible alternatives concrete before a decision.
- `/skill:maestro-documentation` reviews technical guides, comments and API documentation.
- `/skill:maestro-humanizer` revises prose with stock patterns while preserving its claims.
- `/skill:maestro-writing-for-agents` authors or prunes skills and agent rules files.

- `/skill:maestro-idea-refine` helps the human widen and select an early idea.
- `/skill:maestro-wizard` authors a guided Bash procedure for person-only steps.
- `/skill:maestro-to-questionnaire` helps the human request decisions or facts from someone else.
- `/skill:maestro-teach` starts a human-requested learning workspace.

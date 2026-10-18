---
name: maestro-retro
description: "Use when a completed coding session needs improvements to its working environment."
disable-model-invocation: true
---

# Improve the environment from observed mistakes

Read the actual session sources, defaulting to the current session. Suggest environment improvements rather
than retelling the feature work. Rank candidates by severity and cite the incident that makes each worth doing.
Use [forensics](FORENSICS.md) when a deeper transcript investigation is useful.

## Look for repeatable improvements

- Navigation: missing pointers, hidden dependencies and time spent finding information.
- Checks: mechanical mistakes a deterministic lint, type check, test or filesystem check can catch.
- Standards: judgement calls the reviewer missed or ambiguous existing rules.
- Rules files: large `AGENTS.md` instructions that belong in a check, a standard or a referenced document.
- Tool economy and access: expensive calls, missing logs or read-only information that would shorten diagnosis.

Read existing check commands, hooks and CI before proposing another check. A check that exists but is unwired
or broken needs repair, not reinvention. A repository with no guardrail is itself a candidate finding.
For mechanical mistakes, prefer a deterministic check over a prose rule. Reserve standards for judgement calls.
Prefer checks before standards and standards before rules-file edits; keep always-loaded instructions sparse.

Remove no-op instructions that do not change behaviour. Put detailed guidance in reference docs and skills,
with navigation pointers where needed. Distinguish implementation's context pressure from review's ability to
inspect a fixed diff; place judgement guidance with the reviewer rather than burdening every coding turn.

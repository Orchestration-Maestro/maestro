---
name: maestro-writing-for-agents
description: "Use when skills, AGENTS.md or other agent-facing instructions need writing or pruning."
---

# Write instructions agents can reach and use

A context pointer names out-of-context material and the condition for reading it.
Its wording controls discovery. Put the leading trigger word early and name each
distinct branch once; synonyms for one branch do not create useful coverage. If a
required document is missed, sharpen its pointer before inlining the whole document.
For skill packaging, read [skill mechanics](SKILL-MECHANICS.md).

## Place information by need

Always-loaded context costs tokens and attention every turn. Human lookup costs the
reader effort to remember what exists and when to reach it. Spend context on material
the agent must discover; preserve human choice where judgment belongs to the user.

Use three levels: ordered in-file steps, in-file reference consulted as needed, and
disclosed reference in another file behind a pointer. Inline what every branch needs;
disclose what only some branches need. A flat peer set of review rules can remain
in-file. Progressive disclosure protects the instruction hierarchy, not only tokens.
Keep a concept's definition, rules and exceptions together under one heading.
Co-location prevents a reader seeing the rule without the caveat.

## Make completion checkable

Every step ends with a criterion that distinguishes done from not done. Specify both
clarity and demand: 'every changed model accounted for' requires legwork that 'produce
a list' does not. Exhaustive criteria apply to reference documents as well as recipes.
First sharpen a fuzzy bound. If observation still shows agents rushing toward later
steps, split the sequence at a real context boundary such as a handoff or subagent.
An inline invocation leaves the later steps in context and does not hide them.

Split by invocation only when the new skill has independent discovery value. A long
file alone does not justify another always-loaded description; use disclosed reference
for branch-specific details. Keep pointers close to the steps that need them.

## Use positive leading words

A leading word is a compact familiar concept, such as 'frontier' or 'tracer bullet',
that anchors a behaviour. Define a new term when needed, but prefer existing domain
vocabulary. Repeat the token where useful, not a whole paragraph of its definition.
State the desired action positively. A prohibition earns its place when a specific
observed failure needs a hard guardrail; pair it with what to do instead. Copying
bans into every paragraph increases load without making the target clearer.

## Preserve authority and one source of truth

Repository rules govern the work; the approved spec defines the outcome. Source and
configuration show the current implementation. Error output is fresh evidence about
what ran. History explains intent but may be stale. Surface conflicts between these
layers instead of letting a convenient old note override a current contract.

Keep each meaning in one authoritative place. Cache environment facts only when the
lookup is expensive or the fact cannot be found there: an unwritten convention, a
reason or a gotcha. Command lists, directory inventories and config restatements rot
when their owning files change. Link to the owner rather than creating a second one.

Prune stale branches and sentences that never affect the task. Run the document using
[testing guidance](TESTING.md) to identify no-op instructions: if removing a sentence
does not change the observed behaviour, remove it rather than polishing it. Compare
against the actual model's default; a debate about what it 'should' do is not a test.

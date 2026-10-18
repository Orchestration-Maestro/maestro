---
name: maestro-grilling
description: "Use when the ask is underspecified, the user asks to be grilled or stress-test an idea, requirements are being filled in silently, or an approach has a clear problem."
---

# Establish shared intent

Offer a quick reconnaissance of the code before interviewing when repository facts would narrow the questions.
Start with one sentence stating your hypothesis about the desired outcome and an honest confidence estimate.
Name what is unresolved; confidence without a reason does not help the user correct you.

## Work the design tree

Map decisions and their prerequisites. The frontier is the set whose prerequisites are settled.
Ask the whole current frontier in one `ask_user_question` call per round, numbering the questions and putting
its recommended answer first. Phrase recommendations so that “yes” accepts them. Wait for answers, then
recompute the frontier. Questions depending on an unsettled answer belong to a later round.

Finding facts is your job. Inspect sources yourself or use a fresh `subagent` with a
[self-contained brief](../../references/subagents.md). While it runs, ask independent frontier questions;
only downstream decisions wait. The user decides trade-offs, not facts you can look up.

## Test the intent

Play back your understanding in short chunks and invite correction. Surface assumptions rather than silently
completing requirements. When an answer is about convention or what the user “should” want, ask what they
would want if they did not need to justify it. A concrete outcome beats “scalable” or “best practice”.

If the proposed approach has a clear problem, name the issue and its cost, propose an alternative and accept
the user's informed decision. Do not quietly replace their goal with your preferred design.
When terms or durable decisions settle, use `/skill:maestro-domain-modeling` to update the existing glossary or ADRs.

## Confirm and stop

The interview ends when the frontier is empty, with no branch silently assumed. Restate:

- Outcome: what will change.
- User: who benefits.
- Why now: what makes this worth doing.
- Success: how we will check it.
- Constraint: the binding limit.
- Out of scope: what will not be built.

Ask for an explicit yes to this restatement. Delegation, silence or a polite exit is not confirmation.
Fold corrections into the restatement. Once confirmed, deliver it and stop; let the user choose the next phase.

For factual investigation use `/skill:maestro-research`; for alternatives that differ visibly,
use `/skill:maestro-show-options` before asking for the choice.

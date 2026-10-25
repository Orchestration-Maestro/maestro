# Test guidance against behaviour

This is a method for evaluating documents, not an installed runner. Use
`/skill:maestro-tdd` for the baseline-first reasoning and the
[subagent contract](../../references/subagents.md) for fresh-context trials.

## Choose the right observation

- Discipline: expose a realistic temptation to skip a costly requirement and observe
  the choice, not a recital of the rule.
- Technique: give a task requiring the technique, including a missing-information case.
- Pattern: test recognition of a fitting case and a plausible case that does not fit.
- Reference: test retrieval and correct application of the needed fact or syntax.

## Baseline, guidance, revision

1. Define the failure and a realistic scenario before editing guidance. Include actual
   artifacts, available tools, constraints and a required action. Combine pressures
   such as time, sunk cost, authority and fatigue when testing discipline. Do not make
   compliance the obviously advertised answer to a quiz.
2. Run a fresh subagent without the guidance. Record its choices and rationalizations
   verbatim. A baseline that never exhibits the failure cannot show that the new
   guidance prevents it; improve the scenario or record it as already covered.
3. Add only guidance addressing the observed failure. Run the same scenario in a fresh
   context with the guidance. Compare actions, output and missed information against
   predeclared criteria, not against whether the wording flatters the document.
4. Record new evasions and revise the specific weak pointer, criterion or rule.
   Re-test after revision, including earlier successful cases to catch regressions.

Retain the exact prompt, model, available context, artifact version, choice, evidence
and judgment for each trial. A replacement run is not the previous run's result.
Use repeated samples where variability matters; one compliant answer is weak evidence
of reliable behaviour. Review scored matches manually: a keyword match can reward
an answer that says the right thing and does the wrong thing.

## Wording micro-tests versus pressure scenarios

A wording micro-test compares alternative pointers or instruction phrases on the same
small discovery question. Use fresh contexts and repeated samples, preserving the
prompt and scoring rule. It measures wording sensitivity, not end-to-end compliance.

A full pressure scenario requires the agent to act on a real-looking task with combined
constraints and consequences. Observe whether it reaches the right artifact, respects
missing information and finishes the criterion under pressure. Do not replace this
with 'what does the skill say?' Keep micro-test and scenario results separate.

## Run the document

Walk through every instruction and note what changes in behaviour. If a line merely
repeats the model's default, remove the no-op sentence and repeat the test. Check each
reference pointer on the branch that should trigger it. Match exclusions to actual
failure classes rather than adding a general ban for every rationalization. Capture
uncertainty and failed trials; do not claim a campaign ran merely because this file
explains how one would work.

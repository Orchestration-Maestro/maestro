# Diagnostic techniques

## Trace backward

Start at the wrong value or event and trace each producer and caller until the first invalid state appears.
Ask what supplied the value and why it was accepted. Fix that origin, then rerun the original consumer path.
Temporary stack or value probes should be tagged, narrowly scoped and removed after diagnosis.

## Defense in depth

After correcting the origin, identify the boundaries where invalid state could enter again.
Use entry validation, domain invariants, environment guards and diagnostic evidence where each protects a
separate boundary. Do not pile redundant symptom checks onto a path whose actual producer remains wrong.

## Wait for conditions

A fixed sleep guesses when an operation finishes. Observe its completion signal or wait for the actual
condition instead. Keep any genuine production deadline separate from completion. Select the signal that
proves the state the consumer needs, not an earlier intermediate event.

## Bisect polluters

When a test passes alone but fails in a suite, run it with subsets of its predecessors. Split the candidate
set, retaining the half that still causes failure, until one polluting transaction remains.
Check global state, environment, caches, files and cleanup ownership. Keep order and shared state in the witness;
repeating a lone test cannot prove another test does not pollute it.

## When reproduction is missing

Separate an environment mismatch, missing input, race, shared-state leak and external-service change.
Compare versions and configuration; request a redacted trace or the environment that exhibits the symptom.
For intermittent failures, measure repeated observations under controlled stress. For a known-good history,
bisect with the same symptom check. If evidence is insufficient, report what was tried and ask for the missing
access or instrumentation permission. Do not convert a guess into a fix.

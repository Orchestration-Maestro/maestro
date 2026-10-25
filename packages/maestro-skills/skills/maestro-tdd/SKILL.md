---
name: maestro-tdd
description: "Use when building a feature test-first, reproducing a bug, or working through a red-green cycle."
---

# Drive one behaviour through red and green

## Discover the stack and agree the seam

Read the repository's test/build configuration, commands, glossary and relevant ADRs before choosing a framework.
Find existing tests and follow their layout. Agree public test seams with the user, stating what each catches
and misses. Use `/skill:maestro-codebase-design` when the interface or seam is itself uncertain.
Tests describe behaviour a caller needs; they should survive an internal rewrite.
Read [tests](tests.md) and [mocking](mocking.md) before and during the loop.

## Red

Choose one behaviour. Write one small test at the agreed seam and run it before implementation.
Observe failure for the intended missing behaviour, not syntax, setup or a missing dependency.
A test that is green immediately exercises existing behaviour: correct it so it proves the intended missing
capability before proceeding. Do not manufacture a production defect to make a test red.

Delete implementation written before its test and start again from the test; keeping it nearby biases the test.
For a bug, first write and prove a failing reproduction of the actual symptom.
A fresh `subagent` may write that reproduction with the [brief](../../references/subagents.md), but you must
read and run its result. A report alone does not establish red.

## Green

Write only enough implementation to satisfy this test. Run it and check the intended behaviour now passes.
Do not anticipate later tests or write all tests before implementing any behaviour.
Then choose the next behaviour and repeat the vertical loop. Red/green is one slice at a time.

Tests through private details, internal mocks and expected values recomputed by the implementation give
false confidence. Name the user-visible break and use independent expected observations.

## Finish

Keep the full suite green before claiming completion. Refactoring belongs to `/skill:maestro-code-review`,
not an unobserved cleanup phase inside the red/green loop. Preserve the tests' behavioural guarantees when
changing structure. If a test fails only because internal structure changed, reconsider whether it crosses the seam.

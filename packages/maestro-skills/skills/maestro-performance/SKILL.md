---
name: maestro-performance
description: "Use when performance targets exist, users report slowness, a regression is suspected, or profiling identifies a bottleneck."
---

# Measure, change, keep or revert

## Set the target and baseline

Read the project's existing budgets and previous attempts first. For a regression, measure the original
symptom and bisect before changing code. Select the metric users feel: LCP, INP, p95 latency or another
observed result. Use representative devices, networks, data and traffic, not only the developer machine.

Use controlled synthetic measurements to isolate regressions and real-user measurements to confirm the
experience in the field. Record command, conditions, cache state, fixed sampling budget and variance.

| Web metric | Good | Needs improvement | Poor |
| --- | --- | --- | --- |
| LCP | <=2.5 s | >2.5 through 4 s | >4 s |
| INP | <=200 ms | >200 through 500 ms | >500 ms |
| CLS | <=0.1 | >0.1 through 0.25 | >0.25 |

Investigate TTFB above 800 ms. These bands guide investigation, not an assertion about this project.

## Find the bottleneck

For initial loads, read the network waterfall: DNS, connection/TLS, server waiting, blocking CSS/scripts,
images and bundle size. For slow interactions, profile main-thread tasks, rerenders and forced layout.
For navigation, separate API waterfalls from client rendering. For backend slowness, inspect query plans,
connection waits, CPU, memory, lock contention and external dependencies.

Choose the specific fix from [optimization patterns](optimization-patterns.md). A slow request alone
does not prove a missing index, and a large pool does not prove useful concurrency.

## Change one thing

Fix only the measured bottleneck. Re-measure with the baseline's command, conditions and sampling budget.
Repeat enough to distinguish the improvement from run-to-run variance. A hypothetical 3% change inside
5% variance is inconclusive, not a win.

| Result | Action |
| --- | --- |
| Meets the target beyond noise and correctness tests pass | Keep, with before/after evidence |
| Within noise or no measurable improvement | Revert |
| Worse, or any required behaviour regresses | Revert |

Never obtain a faster result by removing validation, skipping required work or weakening a test. If several
changes must ship together, measure each separately before combining them.

Log every attempt, including reverted work, in the PR or the project's performance ledger: idea, baseline,
result, variance, verdict and reason. Mark illustrative measurements as hypothetical; do not invent a result.
The next investigation should read this ledger rather than repeat a failed experiment.

## Guard the result

Use the same user-facing metric for a reproducible synthetic CI budget and field monitoring where applicable.
Compare repeated samples, medians or trends so measurement noise does not become a flaky check. Watch
attributed real-user p75; a rolling aggregate confirms trends but is not an immediate alert.

Illustrative budgets, only when the project has no chosen budget:

| Surface | Example budget |
| --- | --- |
| Initial script | <200 KB gzipped |
| CSS | <50 KB gzipped |
| Above-fold image | <200 KB each |
| Fonts | <100 KB total |
| API latency | p95 <200 ms |
| Interactive time | <3.5 s on 4G |
| Browser performance score | >=90 |

Choose executable checks for the discovered stack and actual surface. Use `/skill:maestro-verify` before
claiming improvement. When a guard fires, establish a fresh baseline instead of reusing an old explanation.

---
name: maestro-ship
description: "Use when preparing a release, enabling a feature, planning staged rollout or rollback, or retiring an existing capability."
---

# Ship with an observed rollback path

## Pre-launch checklist

Use the existing quality bar and `/skill:maestro-quality-gates`. Confirm:

- Required tests, build, lint, types and reviews pass; expected errors are handled and temporary diagnostics removed.
- Inputs, authentication/resource authorization, headers, CORS, auth rate limits, secret handling and dependency
  audit are ready. No unmitigated high/critical dependency finding remains.
- Performance budgets, relevant query plans, caching and responsive image delivery are verified.
- Keyboard/screen-reader flows, contrast, focus and field errors are usable where applicable.
- Production configuration, secrets, migrations, DNS/TLS, static delivery, telemetry and health checks are ready.
- Setup, API and user docs reflect the release. Follow the repository's ADR and generated-changelog policy.

Build instrumentation with `/skill:maestro-observability`. Record the baseline before rollout.

## Flags and staged release

Deploy with the feature off; test both flag states in CI. Give every flag an owner and expiry, avoid nested
flags and remove the old path and flag within two weeks of full rollout. Deployment and user release are
separate decisions.

The following is an advisory rollout example, not evidence that any stage ran:

1. Test staging, including the full required suite and critical-flow smoke test.
2. Deploy to production with the flag off; check health and new errors.
3. Enable internally and monitor for 24 hours.
4. Canary to 5%, compare with baseline for 24-48 hours and advance only on acceptable signals.
5. Increase through 25%, 50% and 100%, retaining the ability to step back at each stage.
6. Monitor full rollout for a week, then clean up.

| Metric | Advance | Hold and investigate | Roll back |
| --- | --- | --- | --- |
| Error rate | Within 10% of baseline | 10-100% above | Above 2x baseline |
| P95 latency | Within 20% of baseline | 20-50% above | Above 50% above baseline |
| Client errors | No new type | New errors in <0.1% of sessions | New errors in >0.1% |
| Business metric | Neutral or positive | Decline <5% | Decline >5% |

These are advisory entries, not a complete classifier for unspecified equality cases. Agree operational
thresholds for the service before release. Roll back immediately for data-integrity problems, a discovered
security vulnerability or a spike in user-reported failures as well as the quantitative rollback signals.

## Error budget and monitoring

Apply the service's error budget: above 20% remaining permits normal monitored releases; 0-20% calls for
slow rollout and no high-risk changes; exhaustion freezes features in favour of reliability. On reset, resume
with the reliability fix retained. Canary burn faster than baseline is a hold signal.

Watch application errors, p50/p95/p99, volume, active users and business outcomes; dependency latency and
errors; CPU/memory, pool usage, disk, network and queue depth; and browser vitals, client errors and API failures.
Report errors with safe context, not credentials or full personal data. Return generic server errors.

In the first hour, check health, new error types and latency; manually exercise the critical flow, confirm logs
arrive and verify the rollback mechanism, preferably in a dry run.

## Plan rollback before deploy

Illustrative rollback targets to verify for your service: feature flag <1 minute, redeployment <5 minutes,
and database rollback <15 minutes. These are examples, not observed project results.

Write triggers, accountable operator, exact verified flag/redeployment steps, health/metric checks and team
communication. State the measured rollback time rather than inventing one. Explain migration reversibility
and what happens to newly written data. A code revert does not undo a destructive schema or data change.
Test the down path and document any irreversible action before proceeding.

## Deprecation and migration

Decide whether removal is justified: unique value still provided, number of consumers, working replacement,
per-consumer migration effort and ongoing maintenance cost. Prefer advisory deprecation for a stable old
system. Compulsory removal needs a justified risk/cost, deadline, tooling, guide and support.

Prove the replacement covers critical use cases in production before announcing removal. Publish status,
reason, replacement, migration steps and removal policy. The owner of deprecated infrastructure owns migration
help; do not leave users to discover missing behaviours. Migrate one consumer at a time, verify equivalent
behaviour and remove its old references. Confirm zero active usage through telemetry and dependency analysis
before deleting code, tests, configuration, docs and obsolete notices.

Choose the appropriate migration shape:

| Shape | Use |
| --- | --- |
| Strangler | Route incrementally from old to new until old traffic is zero |
| Adapter | Preserve the old interface while translating to the new implementation |
| Flag | Move individual consumers with a reversible switch |
| Expand, migrate, contract | Add the new shape, populate it, switch users, then remove old shape separately |

For schemas, add a nullable column, dual-write, backfill in bounded batches, switch reads while retaining both
writes, then stop old writes and drop the column in a later separate deploy. Build large indexes without
blocking normal writes where supported. Test reversal; isolate destructive operations from additive deploys.
Use the same expand-contract shape for a wide refactor. Give unowned but actively used zombie code a maintainer
or a concrete retirement plan rather than leaving vulnerable code in limbo.

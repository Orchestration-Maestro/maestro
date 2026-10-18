---
name: maestro-quality-gates
description: "Use when a project needs a written quality bar, reproducible check commands, CI feedback or a review of weakened checks."
---

# Make the quality bar executable

## Discover before asking

Read and follow an existing bar rather than replacing it. Otherwise inspect the stack manifests, test runner,
linters, current measurements, rules files, hooks and CI. Ask only unresolved choices through `ask_user_question`:
which dimensions matter, block versus warn, chosen targets versus measure-and-hold, and acceptable feedback cost.
In unattended work, apply the floor below, report that choice and leave unresolved dimensions for a human;
do not conduct an interview without a person present.

Record the agreed bar in root `CONSTRAINTS.md` and add one pointer line in the applicable rules file. Include
review date/owner; dimension, threshold and rationale; exact executable command; run placement; measured-only
values with direction; and exceptions with scope, reason, owner and expiry. Convenience scripts mirror this
file, not a second competing bar. Do not invent commands for an undiscovered stack.

## Floor: inspect weakening, not only the result

Review the complete change relative to its merge base, including added/removed lines, staged and untracked
content. Check these five moves:

1. Lowered thresholds or removed checks, including moving a required check out of its phase.
2. Easier assertions, skipped tests or deleted test coverage without a justified reason.
3. Suppressed type, lint, coverage or security checkers.
4. Unimplemented stubs, unfinished replacements or errors silently swallowed.
5. An exception added or broadened without approval.

Tightening is not weakening. Surface a weakening with file/line and rationale rather than silently accepting
new green output. Redact secrets from findings. Distinguish clean inspection, a detected violation and an
inspection unable to run; unavailable tooling is never a clean result. Teach these as language-neutral review
checks; do not add a custom guard program just to restate them.

## Defaults only when the user has no chosen numbers

| Dimension | Default | Reason |
| --- | --- | --- |
| Changed-line coverage | >=80% | Requires meaningful tests without demanding every configuration line |
| Project coverage | Today's measured value, must not fall | Adopt a reachable baseline |
| Optional mutation score | >=60% initially, 80% mature | Start with the suite's actual assertion strength |
| Vulnerable dependencies | No high-or-worse finding | Prioritize material risk |
| Browser LCP | <=2500 ms | Good web-vital threshold |
| Browser CLS | <=0.1 | Good visual-stability threshold |
| Accessibility | Zero critical/serious findings | Prioritize severe barriers |
| Exception lifetime | 90 days | Allow planned repair without indefinite waiver |
| Ratchet tolerance | 0.5% | Absorb measurement drift |

These are generic defaults, not observations or newly imposed rules on the current repository. Every numerical
dimension needs a real command, a consuming surface and an owner. Browser checks need a running URL; do not
pretend a CLI or library has one. Follow the project's decision on whether optional assertion-strength checks
are used. Measure and hold when a new threshold would create a permanently red baseline. Ratchet improved
values upward and report regressions against the recorded baseline.

Distinguish external checks (standards, advisories, real-browser measurements), project checks (human-owned
lint and architecture policy) and suite checks (our tests). Include an applicable outside opinion where possible;
self-authored tests alone do not independently validate the bar.

## Place gates by cost

Keep formatting and fast checks in the existing pre-commit hook, with short edit feedback for types, lint and
secrets. Put related tests and changed-line coverage at completion, costly security/integration/end-to-end
checks in CI, and real-page performance/accessibility checks against a preview or staging URL. Reuse coverage
already produced by the test run rather than running tests again just for a number. Scope expensive checks to
the change where valid. Install no particular hook framework or package manager by default.

Feed exact CI failures back: job, command, exit, relevant output, affected file and commit. Reproduce the failing
stage and repair the cause, not the check. Protect required checks and independent reviews. Keep a build-cop
owner for broken mainline builds and repair or revert promptly rather than allowing red to become normal.

Use previews for review, flags to separate deploy from release, staging smoke tests and a verified rollback.
Keep development, staging and production configuration/secrets separate. Do not put even test credentials in
committed recipes. Promote tested artifacts through environments rather than rebuilding unknown inputs.
Mechanical mistakes should become deterministic checks after looking for an existing owner/check first.

Use `/skill:maestro-security` for boundary controls, `/skill:maestro-performance` for measured budgets and
`/skill:maestro-verify` for honest completion evidence. Record which commands actually ran and which were
unavailable instead of turning a written threshold into a claimed pass.

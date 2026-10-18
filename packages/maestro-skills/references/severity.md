# Grade findings by effect

- **Critical:** security exposure, data loss, broken essential functionality or a defect that makes the delivery unsafe.
  Fix before proceeding.
- **Important:** a real functional gap, architectural problem, unhandled error or missing protection against a credible
  failure. Fix before treating the work as ready.
- **Minor:** polish or a low-impact improvement that does not stop normal use. Record and defer deliberately.

Calibrate against the effect on a reasonable user, not the number of lines or the reviewer's preference.
A vision spec need not enumerate every realistic condition to make a harmful result a finding.
Each item names a location, the defect, its consequence and a remedy where the remedy is clear.

The first review covers the change. Later rounds receive the prior findings and the fix diff only.
Check the fixes and their changed lines; report unrelated discoveries as out-of-scope observations rather than
reopening the full audit. Record deferred Minor findings in the run ledger and point the final reviewer at them.
A deferral is visible work, not a silently dropped finding.

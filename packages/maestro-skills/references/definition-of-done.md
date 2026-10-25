# Definition of done

- The requested outcome works through the intended interface, including realistic errors and edge cases.
- Acceptance criteria have an observation, not only an implementation claim.
- Relevant tests, type checks, lint and build commands pass; documentation matches the delivered behaviour.
- No debug instrumentation, accidental scope, secrets or unreviewed changes remain.
- Review findings have explicit dispositions; concerns and deferred work are visible.

For every passing claim identify the exact full command, run it fresh, read all output, and record its exit
code and pass/failure counts. A filtered run proves only its selection; lint does not prove tests; an agent's
report does not prove its code. Do not claim success from an incomplete or interrupted run.

Check the final diff against the contract as well as checking the test results. If a criterion cannot be verified,
name the missing evidence and do not mark it done. Completion uses the [report statuses](report.md).

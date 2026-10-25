# Brief a fresh subagent

Use a child for a bounded, independently answerable task. Give it a fresh context and a self-contained brief:
goal, actual source/artifact paths, relevant contract, constraints, allowed edits, test expectations and output.
Include the context it needs; do not make it reconstruct the parent conversation or read an entire unrelated plan.

Independent investigations may run in parallel when they do not edit shared state. Give each a distinct failure
domain and ownership. Dependent tasks wait for their prerequisites. Integrate reports and check their evidence;
a confident report is not proof that the resulting diff works.

For independent review, choose a more capable model where available and give it the artifact and contract,
not the builder's conclusions. Keep review read-only. A child never starts children: it reports missing context
or a need for another investigation to its parent instead of recursively dispatching.

Require a report of changed files, commands and actual results, concerns and unresolved questions using the
[completion contract](report.md). Join independent results before claiming the whole task finished.

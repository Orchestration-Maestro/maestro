# Investigate a session from evidence

Locate the actual transcripts and tool logs for the named session; do not infer the sequence from memory.
Read repository changes and timestamps alongside them. Cite each finding by source path and line, separating
what the record shows from your hypothesis about why it happened.

Optional independent `subagent` analysts can examine distinct questions: wasted work, plan drift, repeated
mistakes, missing information or conflicting instructions. Give each a bounded
[brief](../../references/subagents.md); collect and reconcile their evidence rather than counting agreement.

Keep original evidence intact. Assemble a small case with the incident, supporting excerpts, effect, proposed
change and how its effect can be checked. Redact credentials, private paths, personal data and unrelated content
before sharing. Inspect the redacted bundle; an automated pass is not proof it is safe.
Report to the user or the project's chosen tracker, not an external pack maintainer.

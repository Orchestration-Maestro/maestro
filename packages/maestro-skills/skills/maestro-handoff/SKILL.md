---
name: maestro-handoff
description: "Use when work must continue in another session, directory or collaborator context."
disable-model-invocation: true
---

# Save a portable continuation

Read [phase boundaries](../../references/phase-boundaries.md) before choosing to leave
this context. Tailor the handoff to the user's continuation arguments.

Write one file in the OS temporary directory, outside the workspace. Resolve `$TMPDIR`
with `/tmp` fallback on Unix, or `%TEMP%` on Windows. Give it a distinct name and report
the absolute path. The recipient must be able to reach its artifact pointers.

Include the goal, current state, decisions and reasons, open items, next concrete step
and suggested skills. Retain the working directory, branch/head, dirty-file state and
verification commands with actual results. Link specs, reports and artifacts rather
than copying them. Redact credentials and secret values; describe how authorized
readers can obtain them without putting them in the handoff.

If background continuation was requested, start a background `subagent` with the
handoff path and bounded task. Do not launch another agent executable. Otherwise
return the file path and continuation instruction without starting work implicitly.

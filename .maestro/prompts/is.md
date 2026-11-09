---
description: Analyze bug reports and feature requests from GitHub issues
argument-hint: "<issue>"
---
Analyze these GitHub issues: $ARGUMENTS

For each issue:

1. Before analysis starts, add the `in-progress` label to the issue with the GitHub CLI. Use the existing triage roles. If adding the label fails, report the failure and continue.
2. Read the whole issue: every comment and every linked issue or PR.
3. Trust no analysis written in the issue. Verify the behavior yourself and derive your own analysis from the code and its execution path.

4. **For bugs:**
   - Ignore the root cause analysis in the issue; it is likely wrong.
   - Read every related code file in full, without truncation.
   - Trace the code path and identify the actual root cause.
   - Propose a fix.

5. **For feature requests:**
   - Verify any implementation proposal in the issue before you rely on it.
   - Read every related code file in full, without truncation.
   - Propose the most concise implementation approach.
   - List the affected files and the changes each needs.

Do NOT implement unless explicitly asked. Analyze and propose only.

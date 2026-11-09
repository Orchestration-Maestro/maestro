---
description: Finish the current task through release-metadata audit, commit and push
argument-hint: "[instructions]"
---
Finish this task end to end.

Extra instructions: $ARGUMENTS

Work out the context from the conversation history first.

Rules for context detection:
- If the conversation already mentions a GitHub issue or PR, use that context.
- If the work came from `/is` or `/pr`, assume the issue or PR is already known from the conversation and from the analysis already done.
- If the conversation names no GitHub issue or PR, treat the work as non-GitHub work.

Follow these steps under the repository's existing approval rules. Extra
instructions never bypass a safeguard without the approval those rules require.

1. Audit the generated release metadata of the relevant crates: conventional commits,
   PR descriptions and available release notes. Do not edit CHANGELOG.md.
2. If the task ties to a GitHub issue or PR and this session has not posted a final
   comment yet, draft one in my tone and preview the exact text. Post at most one
   final comment under the existing authorization, with
   `gh issue comment --body-file` or `gh pr comment --body-file`.
3. Stage only the files you changed in this session, by explicit path. Inspect
   `git status` and the staged diff; commit only changes you own.
4. If the task ties to exactly one GitHub issue, put `Closes #<number>` in the
   commit message. If it ties to several issues, stop and ask which to close.
   Without an issue, including PR-only context, do not invent a closing trailer.
5. Check the current branch. On the integration branch, pick an authorized
   feature branch instead of pushing to the integration branch.
6. Run the required checks (`just check` and `just test`, as AGENTS.md prescribes)
   before you commit. Keep the commit hooks. Use a signed conventional commit with
   a subject of at most 71 characters and message lines of at most 80 columns.
7. Push the authorized feature branch and follow the repository's protected PR
   and merge-queue procedure within its existing approval boundary. These
   instructions do not by themselves authorize opening, posting or merging.

Constraints:
- Never stage unrelated files.
- Never use `git add .` or `git add -A`.
- Never bypass hooks or protected delivery.
- For work that is not tied to a GitHub issue or PR, do not post a GitHub comment.
- If this session already posted a final issue or PR comment, post no other unless I explicitly ask.
- Reuse the known conversation context; stop only for genuine ambiguity, never to ask again
  for an issue or PR already supplied.

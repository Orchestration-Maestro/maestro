---
description: Finish the current task end-to-end with changelog, commit, and push
argument-hint: "[instructions]"
---
Wrap it.

Additional instructions: $ARGUMENTS

Determine context from the conversation history first.

Rules for context detection:
- If the conversation already mentions a GitHub issue or PR, use that existing context.
- If the work came from `/is` or `/pr`, assume the issue or PR context is already known from the conversation and from the analysis work already done.
- If there is no GitHub issue or PR in the conversation history, treat this as non-GitHub work.

Follow these steps under the repository's existing approval rules. Additional
instructions do not bypass safeguards without the approval those rules require.

1. Audit generated-release metadata for the relevant crates: conventional commits,
   PR descriptions and available release notes. Do not edit CHANGELOG.md.
2. If this task is tied to a GitHub issue or PR and a final comment has not already
   been posted in this session, draft it in my tone and preview the exact text.
   Post at most one final comment under the existing authorization, using
   `gh issue comment --body-file` or `gh pr comment --body-file`.
3. Stage only files you changed in this session using explicit paths. Inspect
   `git status` and the staged diff; commit only owned changes.
4. If this task is tied to exactly one GitHub issue, include `Closes #<number>`
   in the commit message. If tied to multiple issues, stop for an explicit choice.
   Without an issue, including PR-only context, do not invent a closure trailer.
5. Check the current branch. If on the integration branch, select an authorized
   feature branch instead of pushing directly to the integration branch.
6. Run required checks (`just check` and `just test` as prescribed in AGENTS.md)
   before committing. Keep commit hooks; use a signed conventional commit with
   a subject at most 71 characters and message lines at most 80 columns.
7. Push the authorized feature branch and follow the repository's protected PR
   and merge-queue procedure under its existing approval boundary. These
   instructions do not independently authorize opening, posting or merging.

Constraints:
- Never stage unrelated files.
- Never use `git add .` or `git add -A`.
- Never bypass hooks or protected delivery.
- If this is not GitHub issue or PR work, do not post a GitHub comment.
- If a final issue or PR comment was already posted in this session, do not post
  another one unless I explicitly ask.
- Reuse known conversation context; stop for genuine ambiguity, not to ask again
  for an issue or PR already supplied.

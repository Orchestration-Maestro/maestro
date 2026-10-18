# Detect the project's tracker

Read `docs/agents/issue-tracker.md` and `AGENTS.md` first. Their tracker and role mapping win.
Otherwise inspect `git remote -v`: use GitHub through `gh` or GitLab through `glab`.
When neither applies, keep local Markdown under `.scratch/<feature>/`. No setup skill is required.

The five default triage roles are `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human` and `wontfix`.
Use labels with those names unless the repository maps roles differently. Respect that mapping when publishing
or changing state; do not create a parallel label scheme.

## GitHub

- Create: `gh issue create --title "..." --body-file <file>`.
- Read: `gh issue view <number> --json number,title,body,labels,comments`.
- List: `gh issue list --state open --json number,title,body,labels,comments`, adding appropriate filters.
- Comment: `gh issue comment <number> --body-file <file>`.
- Label: `gh issue edit <number> --add-label <label>` or `--remove-label <label>`.
- Close: `gh issue close <number> --comment "..."`.

For parent/child links, use the installed CLI's supported sub-issue commands. The API route is
`gh api --method POST repos/<owner>/<repo>/issues/<parent>/sub_issues -F sub_issue_id=<child-db-id>`.
If unavailable, link the child in the parent's task list and put `Part of #<parent>` in the child.
For a blocker, first get its database id with `gh api repos/<owner>/<repo>/issues/<blocker> --jq .id`, then use
`gh api --method POST repos/<owner>/<repo>/issues/<child>/dependencies/blocked_by -F issue_id=<blocker-db-id>`.
The database id is not the issue number or node id. Query the native dependencies to find unresolved blockers.
When native dependencies are unavailable, put `Blocked by: #<number>` in the child and check those issues' states.

## GitLab

- Create: `glab issue create --title "..." --description "..."`.
- Read: `glab issue view <number> --comments`.
- List: `glab issue list -O json`, adding appropriate filters.
- Comment: `glab issue note <number> --message "..."`.
- Label: `glab issue update <number> --label <label>` or `--unlabel <label>`.
- Close: post the reason as a note, then `glab issue close <number>`.

Put `Part of #<parent>` in a child and link it from the parent. Add a native blocking link with the
`/blocked_by #<blocker>` quick action in a note: `glab issue note <child> --message "/blocked_by #<blocker>"`.
Where the tier lacks native links, keep `Blocked by: #<number>` in the description. Read native links with
`glab api projects/:id/issues/<child-iid>/links`, or resolve the textual references, before claiming a ticket is ready.

## Local Markdown

Create one feature directory, its `spec.md` and one numbered file per ticket in `issues/`.
Read by path; list the directory's tickets. Append comments under `## Comments`, change the `Status:` role to
label or close a ticket, and retain its history. Use `Part of: <parent-path>` and a parent list for child links.
Record `Blocked by: <ticket-path>, <ticket-path>` and treat it as unblocked only when every named ticket is resolved.
Do not replace an existing unfinished spec or combine all tickets into one document.

## Wayfinding operations

Use `/skill:maestro-wayfinder` for the decision-map contract. Reuse the native blocking
operations above, with the existing textual fallback only when native support is absent.
Create all tickets before wiring real ids. In every tracker, the frontier is open,
unblocked and unclaimed; choose first in map order, then number order. A claim is the
session's first write. Store the resolution once in the ticket and append a named
link with a gist to the map's Decisions so far.

- GitHub: create one issue labelled `wayfinder:map`; create children with `wayfinder:<type>`
  and link them through sub-issues or the parent task-list fallback above. List its open
  children and exclude any with an assignee or an open blocker. Claim with
  `gh issue edit <n> --add-assignee @me`. Resolve with `gh issue comment <n> --body-file <file>`,
  then `gh issue close <n>` and update the map.
- GitLab: create a map issue with `wayfinder:map`, children carrying `Part of #<map>` and
  `wayfinder:<type>`. Query children and exclude open blocking links or textual blockers
  and assignees. Claim with `glab issue update <n> --assignee @me`. Resolve with
  `glab issue note <n> --message "<answer>"`, close with `glab issue close <n>` and update the map.
- Local Markdown: use `.scratch/<effort>/map.md` and one numbered child file in `issues/`.
  Keep `Type: research/prototype/grilling/task` and `Status: open/claimed/resolved`.
  Scan in number order, excluding claimed/resolved files and any unresolved `Blocked by`
  paths. Claim by saving `Status: claimed` before work. Resolve under `## Answer`, set
  `Status: resolved` and append a relative named pointer to the map.

## Pull or merge requests as a request surface

Discovery is off by default. Enable it only when the repository explicitly treats
external contributions as requests, following its role mapping and external-author
policy. Filter discovered requests to non-owner/non-member/non-collaborator authors;
an explicitly named request is still handled regardless of author.

GitHub shares issue/request numbers: try `gh pr view <n>` and fall back to
`gh issue view <n>` when it is not a request. Read requests with `gh pr view <n> --comments`
and `gh pr diff <n>`. List through `gh api --paginate 'repos/{owner}/{repo}/pulls?state=open'`,
retaining external authors by `author_association`. Comment with `gh pr comment <n> --body-file <file>`,
label with `gh pr edit <n> --add-label/--remove-label <label>`, and close with `gh pr close <n>`.

GitLab uses separate issue and merge-request number spaces: determine the intended
surface first. Read with `glab mr view <n> --comments` and `glab mr diff <n>`; list with
`glab mr list -F json` and exclude project members/owners. Comment with `glab mr note`,
label with `glab mr update --label/--unlabel`, and close with `glab mr close`.
Local Markdown has no remote request surface; use its issue files instead.

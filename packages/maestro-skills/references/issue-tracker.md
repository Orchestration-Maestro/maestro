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

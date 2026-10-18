---
name: maestro-git-workflow
description: "Use when branches, worktrees, commits, history investigation or branch completion need attention."
---

# Preserve work while using Git

Read the repository's merge, release, signing and generated-file rules first. They
win over this guidance. Inspect status, branch, remotes and existing worktree metadata
before changing the checkout. Establish who owns the workspace and what must survive.

## Detect isolation before creating it

Distinguish an ordinary checkout, an existing linked worktree, a submodule and an
externally managed detached HEAD. Inspect `git worktree list`, the Git directory and
`git rev-parse --show-superproject-working-tree`; a `.git` file alone is ambiguous.
Use the host's native isolation where available and respect the user's location
preference. Reuse suitable existing isolation instead of nesting another worktree.
A managed detached HEAD is not permission to switch branches or remove its workspace.

For a manually created local worktree, prefer the repository's existing convention.
Confirm the directory is ignored with `git check-ignore` before creating it inside
the repository. If it is not ignored, obtain the appropriate ignore change first.
Create a named short-lived branch, install only the project's documented prerequisites
and run its baseline checks. Report permission errors or a red baseline; ask how to
proceed rather than bypassing permissions or treating existing failures as yours.

## Commits and history

Keep branches short-lived and integrate frequently under the repository's policy.
Make atomic commits whose messages explain why the change exists, not just which
files moved. Keep behaviour-preserving refactors in their own commits, separate from
features and fixes, so review and revert remain intelligible.

Before committing, inspect status and the staged diff. Stage only intended files;
exclude secrets, scratch output, build artifacts and unrelated changes. Review
new files and generated output too. Change generation inputs rather than editing
owned generated records; regenerate through the documented command. Run required
hooks and checks, and inspect the resulting diff if formatting changes it.

Use `git log`, `git show` and `git blame` to understand an unfamiliar constraint.
For a regression with a reproducible predicate, use `git bisect`: mark known good
and bad revisions, run the same discriminating check at each step, and end with
`git bisect reset`. Protect uncommitted work before moving history.

Follow the repository's semantic-version policy: patch for compatible fixes, minor
for compatible additions, major for breaking changes unless its documented policy
differs. Tags identify immutable releases; do not move a published tag. Let existing
release tooling own version updates, generated changelogs and release notes.

## Permission boundary

Obtain user confirmation before push or any use of `reset --hard`, `clean -f`,
`branch -D`, `checkout .` or `restore .`. Explain the exact target and effect. A
request to finish a feature is not blanket permission to destroy local work. Stop
on a permission failure rather than retrying through a bypass.

## Finish a green branch

Use `/skill:maestro-verify` before offering completion choices. Report failing tests
and repair them before presenting a green finish. Offer:

1. Merge locally, if repository rules allow it and HEAD is on an owned branch.
2. Open a pull or merge request through `/skill:maestro-pr`.
3. Keep the branch and worktree for later, reporting their locations and state.

At a detached HEAD, omit local merge; explain how the commits will be retained.
Preserve request and keep worktrees. Remove a worktree only if this run created it,
its commits are safe and the chosen completion path no longer needs it. Never delete
an external workspace. If cleanup encounters uncommitted files, refuse forced removal
and surface choices: preserve, commit intentionally, or explicitly authorize discard.

Discard is available only when explicitly requested, not as a suggested fourth menu
option. Explain which commits and files would be lost and require the user to type
`discard`. Confirm any destructive commands separately as required above. Do not
interpret a vague acknowledgement as the typed confirmation.

# Session navigation

`AgentSession::navigate_tree` selects an existing history entry. Selecting a user
message returns its text for the editor and moves the active leaf to its parent;
selecting the first user message leaves the active branch empty. Selecting an
assistant message makes that entry the leaf without returning editor text.
Selecting the current leaf does not append entries.

## Branch summaries

With `NavigateTreeOptions::summarize` enabled, navigation can summarize the branch
being left. The summary becomes the new leaf. For a user target, it attaches to
the user's parent, alongside the selected user; for an assistant target, it
attaches to that assistant. A summary at the first user target has no parent.
Custom instructions reach the summarizer as additional focus unless replacement
instructions are requested.

A `session_before_tree` extension can cancel the move. Once a summary request has
started, `abort_branch_summary` cancels that request. Both cancellation routes
clear `is_compacting`; an aborted summary leaves the retained entries and leaf
unchanged.

## Context usage after compaction

`get_session_stats` reports token totals separately from current context usage.
Immediately after compaction, the current token count and percentage are unknown,
even if a kept assistant message has usage. A new assistant response supplies the
current usage; aggregate totals still include the kept response.

## Qualification

The native integration checks build navigation histories through completed
session prompts. They compare editor text, summary placement, active leaf,
request instructions and cancellation cleanup with recorded observations. See
[testing the application session](testing.md) for the harness and focused command.

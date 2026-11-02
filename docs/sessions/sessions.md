# Recover and branch a session

Run `maestro -c` from the project directory to reopen its most recently modified
saved session. Use `maestro -r` when you need to choose a different one.

## Find the saved conversation

Maestro stores native sessions as JSONL files under
`~/.maestro/agent/sessions/`, grouped by working directory. A project such as
`/work/relay` uses `--work-relay--/` beneath that directory. Files contain a
session header and a tree of entries, not only the currently visible transcript.
Filenames combine a timestamp and the session ID.

A fresh session starts saving once it contains an assistant message; opening
Maestro and setting a name alone does not create a saved conversation. Later
entries append to the file. Keep a copy of the JSONL file before editing or
removing history you need.

`MAESTRO_CODING_AGENT_DIR` changes the agent root. To choose a session directory
instead, use `--session-dir <dir>`,
`MAESTRO_CODING_AGENT_SESSION_DIR`, or the `sessionDir` setting, in that order
of precedence. An explicit directory holds the session files directly; it does
not add a project subdirectory. See [configuration](../application/configuration.md)
and [settings](../settings.md#commands-sessions-and-models).

The following commands assume the `maestro` executable is on PATH. For a source
checkout, see [running the executable](../cli.md#executables).

| Task | Command |
| --- | --- |
| Start another conversation | `maestro` |
| Continue the newest saved file in the selected directory | `maestro -c` or `maestro --continue` |
| Choose a saved conversation | `maestro -r` or `maestro --resume` |
| Open a known file | `maestro --session ./saved.jsonl` |
| Find a session by its full ID or ID prefix | `maestro --session <id>` |
| Copy a saved session into a new session | `maestro --fork ./saved.jsonl` or `maestro --fork <id>` |
| Keep history in memory without saving it | `maestro --no-session` |

Replace `<id>` with the ID shown by `/session`, or a prefix of it. ID lookup is
case-sensitive and searches the current project first, then other projects;
when several IDs match, it takes the first match in discovery order. Use a full
ID or path to avoid ambiguity. A slash, backslash or lower-case `.jsonl` suffix
makes the argument a path rather than an ID.

Continuing an empty session directory starts a new session. Opening another
project's session by ID asks before copying it into the current project;
`--fork` makes that copy without asking. Opening an explicit path uses that file
in place. CLI `--fork` copies the source's stored history, including branches;
it is different from the interactive earlier-message `/fork` below.
Do not combine `--fork` with `--session`, `--continue`, `--resume` or
`--no-session`.

`--no-session` prevents session-history writes, not all application writes or tool
side effects. Settings, credentials and files changed by tools are separate.

## Name work before you leave

In the interactive editor, submit:

```text
/name Retry queue investigation
/session
```

`/name` assigns a display name without renaming the JSONL file. `/name` without
text shows the current name, or usage guidance when none is set. `/session`
shows the name, file path (or `In-memory`), ID, message and tool counts, token
usage and nonzero cost for the current context. These are recorded usage figures,
not an account bill.

Use `/new` to start another session, `/resume` to choose a saved one, and `/quit`
to exit. A session extension can veto a switch; the existing history stays
selected. [Interactive commands](../chat/commands.md) describes the error and
cancellation paths.

## Search, rename or delete saved work

Open `/resume` while chatting, or start with `maestro -r`. The picker initially
shows the current project's sessions. These are default bindings; customized
[keybindings](../terminal/keybindings.md) change the keys and the displayed hints.

| Control | Result |
| --- | --- |
| Type a query | Search ID, name, message text and working directory |
| Tab | Switch between current-project and all-project scope |
| Ctrl+P | Show or hide session-file paths |
| Ctrl+S | Cycle Threaded, Recent and Fuzzy ordering |
| Ctrl+N | Toggle named-only filtering |
| Up/Down; Page Up/Page Down | Move one row; move ten rows |
| Enter; Escape | Resume the selected session; cancel the picker |
| Ctrl+R | Edit the selected session's name; Enter saves, Escape cancels. Only in `/resume`; the `maestro -r` picker ignores it |
| Ctrl+D | Ask to delete the selected session; Enter confirms, Escape cancels |

Search accepts fuzzy terms, quoted phrases such as `"retry queue"`, and
case-insensitive regular expressions prefixed with lower-case `re:`, such as
`re:retry|backoff`. Threaded ordering groups related sessions when the query is
blank; a query ranks matches like Fuzzy ordering. Recent keeps newest-first
ordering while filtering. Named-only filtering excludes blank names. Rename
saves a trimmed, nonblank name. See [search details](../chat/selectors.md#session-search).

Deletion first tries the `trash` executable. If it is unavailable or fails,
Maestro falls back to **permanent file deletion without a second confirmation**.
Back up needed history before confirming. The picker refuses to delete the
currently active session. Ctrl+Backspace also requests deletion when the search
query is empty; with text present, it edits the query instead.

If a resumed session's saved working directory no longer exists, Maestro asks
whether to use the current directory. Decline to leave the current session alone.
See [picker behavior](../chat/session-selector.md#rename-and-delete) for failure
reporting and [discovery](../discovery.md) for directory scope.

## Try another approach without losing the first

Suppose you asked for a queue design, tried a fixed delay, and now want to try
exponential backoff from the same starting point:

1. Submit `/tree`. Find the user message that requested the fixed delay.
2. Select it with Enter. Choose `No summary` to avoid a summarization request.
3. The editor receives that message's text. Replace it with
   `Use exponential backoff for the retry queue.` and submit it to your configured model.
4. Open `/tree` again. Both attempts remain in the same session file. Select an
   assistant answer on the fixed-delay branch to continue after that answer.

The resulting history has two paths after the shared design answer:

```text
user: Design the retry queue.
  assistant: Queue design
    user: Use a fixed retry delay.
      assistant: Fixed-delay proposal
    user: Use exponential backoff for the retry queue.
      assistant: Backoff proposal
```

Only the selected ancestry supplies the conversation context. Switching branches
changes that selection; it does not delete the other path. Selecting a user or
custom message moves to its parent and returns its text to the editor. Selecting
the first user message moves before the conversation. Selecting an assistant,
tool result, compaction or other entry continues after that entry, with no
returned prompt. A nonblank existing draft is kept rather than overwritten by
returned text.

### Find a point in the tree

| Default control | Action |
| --- | --- |
| Up/Down | Move through visible entries, wrapping at the ends |
| Left/Right or Page Up/Page Down | Move one page |
| Ctrl+Left/Right or Alt+Left/Right | Fold/unfold a branch or move between branch segments |
| Type; Backspace | Search visible categories by literal tokens; remove a query character |
| Shift+L | Edit the selected entry's label; save empty text to remove it |
| Shift+T | Toggle label timestamps |
| Ctrl+O; Ctrl+Shift+O | Cycle filters forward; backward |
| Enter | Select the highlighted entry |
| Escape or Ctrl+C | Clear a nonempty query first; otherwise close the tree |

Filters cycle through `default`, `no-tools`, `user-only`, `labeled-only` and
`all`. Default hides bookkeeping such as model changes and session names;
no-tools also hides tool results. Use user-only to find a prompt, labeled-only
to revisit marked decisions, or all to inspect bookkeeping. Direct filter keys
are Ctrl+D, Ctrl+T, Ctrl+U, Ctrl+L and Ctrl+A respectively. Pressing the same
nondefault filter key again returns to default. `treeFilterMode` sets the initial
filter and defaults to `default`; see the [settings reference](../settings.md#display).

Tree search requires every token, ignoring case; it is not the picker's fuzzy or
regular-expression search. Search or filter changes clear folds. Labels attach
to entries, not sessions; use `/name` for the session's display name. See
[tree controls and labels](../chat/tree-selector.md) for category and rendering
details.

### Choose whether to stay in the same file

| Operation | What it keeps | Use it when |
| --- | --- | --- |
| `/tree` | Full history tree in the same file; changes the selected branch | Alternatives belong to one investigation |
| `/fork` | New session containing ancestry before a selected user message; places that prompt in the editor | An earlier prompt needs a separate conversation |
| `/clone` | New session containing the current branch through its exact leaf; clears the editor | The current result is a starting point for separate work |

`/fork` lists user messages with the newest selected. Escape preserves your draft;
a successful fork replaces it with the selected prompt. `/clone` needs a selected
leaf; an empty conversation reports `Nothing to clone yet`. Neither operation
requests a branch summary. See [fork and clone behavior](../chat/commands.md#forking-and-cloning).

## Carry context across a branch switch

After selecting a different tree entry, choose `Summarize` to retain a summary
of the branch you are leaving, or `Summarize with custom prompt` to supply a
focus such as `Keep the retry limits and rejected delay values.` This requests
model work and can incur usage charges. `No summary` navigates without that work.
The summary is attached at the destination; it does not move the abandoned
branch's messages into the selected ancestry.

`branchSummary.skipPrompt` defaults to `false`. Setting it to `true` skips the
choice and navigates without a summary, rather than automatically summarizing.
Cancelling the summary returns to the selected tree entry. `/compact [instructions]`
instead reduces the current context while retaining raw history. Read
[compaction and branch summaries](compaction.md) before changing those policies.

## Inspect or export the records

Use the path from `/session` to locate the JSONL file. The
[session format](session_format.md) describes headers, entry IDs, parent links,
labels, compactions, branch summaries and extension records. For embedding and
history operations, see [SessionManager](session_manager.md),
[labeled history trees](../tree.md) and [message projection](messages.md).

`/export branch.jsonl` writes the current branch as JSONL. HTML export and
`/share` require a viewer bundle; the complete offline viewer remains tracked in
[the viewer issue](https://github.com/Orchestration-Maestro/maestro/issues/1762).
Use [export and import commands](../chat/commands.md#export) for supported paths
and failure behavior. The [settings guide](settings.md) covers session preferences
by topic.

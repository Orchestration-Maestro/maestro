# Session discovery and working-directory diagnosis

`SessionManager::list` and `SessionManager::list_all` summarize stored sessions
without opening them as histories, and `session_cwd` reports a stored working
directory that no longer exists. File framing, record shapes and date parsing
belong to [session history](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/sessions/session_manager.md)
and the [session format](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/sessions/session_format.md);
this page states only what discovery decides. Listing is native-only and needs an
active Tokio runtime owned by the caller; the library never builds one.

## Listing

`list(cwd, session_dir, agent_dir, on_progress)` reads one project directory. An
explicit `session_dir` is used verbatim (and never creates anything); an omitted
one creates and uses the encoded default directory for `cwd` beneath `agent_dir`,
and only a failure creating it is returned as an error. `list_all(agent_dir,
on_progress)` reads `<agent_dir>/sessions/*/`: only real subdirectories
count (files and symlinks to directories are ignored), an unreadable
subdirectory contributes nothing, and an unreadable root gives an empty list.
It never creates the root. Both methods return an empty list rather than an
error for any listing failure, and neither writes a session file.

A candidate is any directory entry whose name ends in exactly `.jsonl`,
symlinks included. Candidates are enumerated first, in name order on every
platform. For `list_all` the project directories come in name order too, and
every directory is enumerated and counted before the first candidate is read.
The candidates are then read in the calling task, at most ten at a time. Each file
is read line by line, so a large session costs about its longest line in memory,
and a file whose reached record is malformed stops being read there.

## Per-session metadata

A file becomes a `SessionInfo` when its first parsed record has `type` `session`,
so the header needs no string `id` (identity and `parentSession` pass through as
supplied or absent). The file fails, and is silently skipped, when it cannot be
read or stat-ed, when a reached record is `null`, or when a selected field has an
unusable type: a `message` record whose `message` member is missing or `null`; for
a `user` or `assistant` message, `content` that is neither a string nor an array
or a `null` content block; or a `session_info` name that is neither a string nor
`null`. A file whose later records are malformed lines
is not failed; those lines are skipped by the shared parser.

- `message_count` counts every `message` record.
- Only `user` and `assistant` messages that have a `content` member are
  eligible. Their text (a string, or the `text` members of text blocks joined by one
  space) is joined across messages by one space; empty text adds nothing to the search
  text.
  `first_message` is the first nonempty user text, untrimmed, else `(no messages)`.
- `name` is the newest `session_info` name, trimmed; an absent, null or blank name
  clears it, and the whole file is scanned forward, so a wrong-typed earlier name
  fails the file.
- `cwd` is the header string or empty; `created` is the header time (`NaN` when it
  is absent or not a date, zero for null).
- `modified` is the greatest activity time of eligible messages when positive,
  independently of whether their text is empty. A
  numeric message `timestamp` is that message's activity (even zero or
  negative); otherwise its record `timestamp` string is parsed. When no positive
  activity exists, a header `timestamp` that is a date-parsable string is used;
  a numeric, null, missing or unparsable header time does not qualify, and the
  file's modification time is used instead.
  Dates are epoch milliseconds; fractional values truncate. Custom and tool
  records never move `modified`, which also makes it differ from
  `continue_recent`, which orders files by modification time.

## Progress and ordering

`on_progress(loaded, total)` runs once for every candidate as its processing completes,
valid or not, in completion order, from the task awaiting the listing. A callback
error stops the listing at once and returns an empty list; reads already in flight
are dropped, not drained, and the callback is never called after the listing
returns. Results are collected in enumeration order and stably sorted by
descending `modified`, so equal times keep name order.

```rust
use maestro_session::SessionManager;

let dir = std::env::temp_dir().join(format!("maestro-discovery-{}", std::process::id()));
std::fs::create_dir_all(&dir)?;
std::fs::write(
    dir.join("a.jsonl"),
    concat!(
        r#"{"type":"session","id":"a","timestamp":"2025-01-02T03:04:05.000Z","cwd":"/work"}"#, "\n",
        r#"{"type":"message","message":{"role":"user","content":"hello","timestamp":2000}}"#, "\n",
    ),
)?;
let runtime = tokio::runtime::Builder::new_current_thread().build()?;
let mut seen = Vec::new();
let mut progress = |loaded, total| {
    seen.push((loaded, total));
    Ok(())
};
let listed = runtime.block_on(SessionManager::list(
    "/work",
    dir.to_str(),
    "unused",
    Some(&mut progress),
))?;
assert_eq!(seen, [(1, 1)]);
assert_eq!(listed[0].first_message, "hello");
assert_eq!(listed[0].modified, 2000.0);
std::fs::remove_dir_all(dir)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Missing working directory

`get_missing_session_cwd_issue(manager, fallback_cwd)` returns an issue only when
the manager has a nonempty session file and a nonempty stored working directory
whose path does not exist; any existing path, a symlink's target included,
counts as present, and the fallback is never probed. `assert_session_cwd_exists`
wraps that issue in `MissingSessionCwdError`, whose message is formatted once
when the error is built. `format_missing_session_cwd_error` and
`format_missing_session_cwd_prompt` interpolate the authored strings
unchanged; the session-file line appears only for a nonempty file. A caller that
passes an existing working directory to `SessionManager::open` avoids the issue;
the override itself rewrites no healthy current header (recovery and conversion
of older files follow the [session history](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/sessions/session_manager.md)). Deciding what to do about an issue belongs to
the application. The records and formatters are available on every target; the
diagnosis reads the filesystem and is native-only.

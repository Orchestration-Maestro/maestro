# Interactive commands

The interactive mode reads submitted editor text before it becomes prompt input.
Text it recognizes as a command runs in the mode and answers on the transcript;
every other submission reaches the [typed input](interactive-mode.md#typed-input)
and the editor history as before.

Today the mode handles three commands, `/export`, `/share` and `/copy`. Import, the
offline viewer and packaged asset injection are not delivered yet (see
[what is missing](#what-is-missing)).

## `/export`

`/export` with no path writes the whole session as an HTML document. A path
ending in exactly `.jsonl` writes the current branch as JSONL lines instead.

```text
/export
/export transcript.html
/export "notes from friday.jsonl"
```

Both formats reuse the [session export](../export.md) owners: the HTML document
comes from the same serializer and native bundle that
`AgentSession::export_to_html` uses, and the JSONL lines come from
`AgentSession::export_to_jsonl`. The mode adds no serializer, no asset fallback
and no second file writer.

A success shows `Session exported to: <path>` in the transcript's dim status
colour, naming the path the export owner returned. JSONL returns the
authored path resolved against the process directory; HTML returns the authored
path, or `maestro-session-<session file name>.html` when no path was given.

A recoverable failure shows `Failed to export session: <diagnostic>` in the
transcript's error colour, with the diagnostic the export owner produced. A
failure to render that notice goes to the terminal runtime's error route, which
is the route the mode's shortcuts already use.

## `/share`

`/share` exports the current session to HTML and publishes it as a secret GitHub
gist, then reports the gist and, when a hosted viewer origin is configured, its
viewer URL. It takes no argument: `/share extra` is prompt text.

```text
/share
```

The command runs in four steps, each with its own answer on the transcript:

1. It runs `gh auth status`. A program that is missing or cannot be executed shows
   `GitHub CLI (gh) is not installed. Install it from https://cli.github.com/`;
   a nonzero or signalled status shows `GitHub CLI is not logged in. Run 'gh auth
   login' first.` Neither starts an export.
2. It writes the session, through the same HTML export owner `/export` uses, into a
   directory this share operation alone owns.
3. It replaces the editor with a cancellable `Creating gist...` loader and runs
   `gh gist create --public=false <path>`, collecting the command's output.
4. It restores the editor, removes its own directory and document, and reports the
   result: `Share URL: <viewer>` and `Gist: <url>` when a viewer origin is
   configured, `Gist: <url>` when it is not, or `Failed to create gist: <message>`
   in the transcript's error colour. A zero exit whose output holds no identifier
   shows `Failed to parse gist ID from gh output`; the identifier is the text after
   the last literal slash, kept exactly as printed.

Cancelling the loader with the cancel key terminates the gist command, restores the
editor, removes the document and answers `Share cancelled`. A failure to export the
session shows `Failed to export session: <message>` and starts no gist. Every
failure after the editor is replaced restores it, and cleanup never replaces the
result.

Two share operations never write the same path: each creates its own directory, and
a directory that already exists is refused rather than reused. The captured output is
decoded once from the complete bytes, so a multi-byte character split across pipe
reads survives.

## `/copy`

`/copy` writes the last answer to the clipboard. It takes no argument.

```text
/copy
```

The text is the session's last answer, trimmed under the shared whitespace policy
(U+0085 is kept), with its text blocks joined without a separator and thinking
omitted. An answer that was aborted with no content is skipped in favour of the
previous one. With nothing to copy the mode shows `No agent messages to copy yet.`
A completed copy shows `Copied last agent message to clipboard`; a reached
clipboard failure shows its own message in the error colour. The mode reads the
current session each time, so a replaced session is copied from its own answer.

## How the path is read

The reader recognizes the command token, then reads one path. It is token
reading, not shell parsing:

- The text must be exactly `/export`, or `/export` followed by one space. A
  prefix such as `/exporter` or `/export` followed by a tab is not the command,
  and that text is delivered as prompt input.
- After that single space, leading whitespace is removed under the shared
  [whitespace policy](../terminal/text.md): Unicode white space other than
  U+0085, plus U+FEFF.
- A quoted path returns the text between the first quote and the next matching
  quote, including empty text. An unterminated quote leaves no path, so the
  command exports HTML under the default name.
- A bare path ends at the first whitespace character. Later tokens are ignored.
- Nothing is unescaped, expanded or normalized: `~/notes.jsonl` and `"notes
  from friday.jsonl"` reach the export owner as authored.

The first matching quote wins, so `"/export/one"` is the path `/export/one` and a
second quote character inside the path ends it. Apostrophes are ordinary
characters unless the path starts with one.

## Which format is chosen

Only an exact lower-case `.jsonl` suffix selects JSONL. `.JSONL`,
`archive.jsonl.bak`, `notes.jsonl-summary` and an absent or empty path all select
HTML. The comparison stays case-sensitive on purpose: the reference command
selects JSONL for that suffix alone.

## What is missing

- Import (`/import <path.jsonl>`) is not delivered. It waits for the shared
  dialog host and the session rebuild helpers, and it stays part of the same work.
- The complete offline viewer is separate work (#1762); the export tests supply a
  controlled bundle, so they prove the serializer wiring and not a shipped viewer.
  Until it ships, a share with no bundle installed follows the export failure path
  above and reports it.
- Packaging and asset injection are separate work (#249). The mode reads the bundle
  the configuration's package directory selects, exactly as the other export
  callers do.

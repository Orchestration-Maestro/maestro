# Changelog sections

The application holds the changelog utility in `maestro_app::utils::changelog` and
re-exports its items at the crate root. It is available on native targets. The
[`/changelog` command](commands.md#changelog) reads it. Startup uses the same reader to select unseen sections.

`parse_changelog` reads a supplied path once and returns the sections in file order.
A path that names no file yields no sections and writes nothing. A path that exists
but cannot be read, such as a directory or a file the process may not read, writes
`Warning: Could not parse changelog: <error>` to standard error and yields no
sections. Bytes are decoded lossily, so invalid UTF-8 becomes replacement text and
an initial byte-order mark is kept.

Text is split on LF alone. A line that begins with a literal `## ` starts a section
boundary; indentation, a tab in place of that space and deeper heading marks do not.
On such a line the first run of `##`, one or more whitespace scalars, an optional
`[`, three ASCII-digit components separated by dots and an optional `]` gives the
section's version. A missing or unmatched bracket, a trailing date or text, a
prerelease suffix and a fourth component all leave the matched prefix admitted, and
a later heading mark on the same line is recognized, including a mark that overlaps
the one that failed to give a version. An admitted line without such numbers closes
the current section and discards the text after it until the next versioned heading.

Section text keeps the heading line and every following line with its internal CR,
CRLF and whitespace, and the section boundaries are trimmed with the shared
whitespace scalars. A leading byte-order mark keeps the first line from matching.

`compare_versions` returns the difference of the first numeric component that
differs: major, then minor, then patch. A positive result means the first entry is
newer, a negative one means it is older, and zero means the numbers match. Section
text never decides the order.

`get_new_entries` selects the entries strictly newer than a saved dotted version.
The first three components are read as numbers; an absent, blank, invalid or
non-numeric component counts as zero, as do the spellings `inf` and `nan`;
`Infinity` and `-Infinity` keep their values. Extra components are ignored, and the
saved text is not validated as a whole version. Selection returns the caller's own
records in their input order, so equal versions keep their distinct entries and the
caller still owns the text.

`get_changelog_path` re-exports the configuration owner's path for a supplied
package directory; it adds no policy of its own.

## Startup history

Before starting the terminal, a fresh session reads the packaged changelog. A
session that already has messages skips the read. An unset or empty saved version
records the installed version without showing history. Otherwise only sections
newer than the saved version are selected, in file order with duplicates intact.
The saved version changes only when sections are selected.

Recording a version starts install reporting through the application's existing
preference, environment and offline policy. `InteractiveModeOptions::service_origin`
and `fetch` supply the hosted origin and transport; the empty compiled origin
sends no request. Reporting errors do not interrupt startup.

After the loaded resources and before session replay, the mode shows its selected
history once. With `collapseChangelog`, it shows an update sentence containing the
first heading version and a bold `/changelog` hint. Otherwise it shows a bordered
**What's New** card with Markdown in the current theme. No history adds no card;
the mode still marks the notice handled.

## Update cards

`show_new_version_notification` appends a warning-bordered card with the supplied
version, the `maestro update` command and a changelog row. Where the terminal
supports links, the row is a link labelled `open changelog`; elsewhere it prints
the changelog address. The card reads link support once, when it is created.
`show_package_update_notification` appends the same framing around the supplied
package names, preserving their order and duplicates. Both methods accept empty
values and request a frame. Borders follow later theme changes; the card text
keeps the theme used when it was created.

## Independent startup checks

After initialization, `run` starts the release, package and terminal checks before
startup warnings and prompts. Notifications run through the terminal's task host,
so synchronous warnings precede even an already-ready result. The checks do not
block prompt input. Results appear
in completion order; absent results and failed requests add no card. Release checks
and install reporting share the supplied origin and HTTP transport.

Package checks build a fresh manager from the current session directory, the
current agent directory and the session's settings handle. The startup offline
selection reaches release checking, reporting and package checking. An omitted
selection uses the owning operation's ambient policy. For the startup package
wrapper, any nonempty `MAESTRO_OFFLINE` value skips all package observations,
including `0`, `false` and a space, when the startup selection is omitted.

Inside tmux, both keyboard options are queried concurrently. The queries keep
native success distinct from signal termination and settle the notice after two
seconds even when native capture cleanup is still pending. Unavailable queries
stay silent. An extended-keys value other than `on` or `always` shows the setup
warning; otherwise `extended-keys-format` equal to `xterm` shows the format warning.
Maestro does not edit terminal configuration.

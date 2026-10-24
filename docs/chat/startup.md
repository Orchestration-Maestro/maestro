# Changelog sections

The application holds the changelog utility in `maestro_app::utils::changelog` and
re-exports its items at the crate root. It is available on native targets. The
startup notices that read it and the command that prints it are not wired yet.

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

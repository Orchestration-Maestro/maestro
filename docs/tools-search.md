# File listing and finding

`maestro-tools` lists a directory with the `ls` tool, built by `create_ls_tool` or
`create_ls_tool_definition`. Both are native-only, like the [read tool](tools-read.md#read-tool),
and the crate still builds for browser targets without them. The native `find` tool
searches file names; content search is not delivered here.

## Arguments

`path` is the directory to list. An absent or empty value means the working
directory the tool was built over. The path is resolved when the tool is called, not
when it is built, by [`resolve_to_cwd`](../crates/maestro-tools/src/path_utils.rs), which
owns the rules for absolute spellings, `~` and the working directory.

`limit` is the number of entries to return and defaults to 500. It is a number, not an
integer: a fractional limit such as 1.5 allows two entries, and zero or a negative limit
returns no entries. The schema declares neither a range nor an integer type.

## Listing

[`LsOperations`](../crates/maestro-tools/src/search/ls/mod.rs) supplies the three file
effects of a listing: `exists`, `stat` (whether a path is a directory) and `readdir` (entry
names). `LsToolOptions::operations` replaces the local filesystem with another system; it
is used instead of the native operations, never beside them, and nothing but these three
effects varies.

A call checks the path exists, then that it is a directory, then reads its names. A missing
path fails with `Path not found: <path>` and a non-directory with `Not a directory: <path>`;
a failure of the existence check or of the directory check keeps its own message, and only
a failure of the directory read is reported as `Cannot read directory: <message>`.

The names are sorted by comparing their lowercase forms with the
[locale collator](theme.md#replaceable-operations), keeping the order of names that compare
equal, so upper and lower case of one letter stay adjacent and digits are not read as numbers.
The collator's locale comes from the process environment, whichever operations list the
directory.

Each name is then examined in that order. An entry whose check says directory gains a
trailing `/`; links are followed by the native check. An entry whose check fails is left
out and does not count toward the limit. No ignore rules apply: dot files, `.git` and
names listed in `.gitignore` are listed. Before examining a name, the call stops when it
already holds `limit` entries: a name that remains after a full listing is never examined,
yet still marks the listing as cut, and a listing with no name left is not marked.

## Result

The result text joins the entries with LF; a directory that yields no entries returns
`(empty directory)` without details. The listing body, before any notice, keeps at most 51200 UTF-8 bytes through
the shared [head selection](tools-read.md#limits-and-selection) and has no line limit.
When the entry limit stopped the listing, a notice
`[<limit> entries limit reached. Use limit=<twice the limit> for more]` follows after a
blank line and `details.entryLimitReached` holds the limit. When the byte limit cut the
text, the notice `50.0KB limit reached` is added inside the same brackets, after `. ` when the
entry-limit notice is also present, and `details.truncation` holds the selection record. Details are absent when neither
limit applied.

A signal aborted before the call, or while an operation is pending, makes the call fail
with `Operation aborted`, and an abort raised during an operation wins over that operation's
own result or error. Dropping the pending operation is the whole cancellation.

## Rendering

The call line shows `ls`, the listed path (`.` when absent, null or empty, a leading `~` for
the home directory or a path below it, `[invalid arg]` when it is not a string) and, when the arguments carry a
`limit`, ` (limit <value>)`, with a present `null` shown as `null`. The result shows the
text trimmed of leading and trailing whitespace, the first 20 lines unless the row is
expanded, the count of hidden lines with the live expand key, and a warning
`[Truncated: <n> entries limit, <size> limit]` that names the entry limit when the details
carry a nonzero one and the byte limit, formatted as `<size>` from the record's byte
budget (50.0KB for a normal listing), when their truncation record says it cut. Only the
result options decide expansion. Both renderers reuse the previous `Text` component of
the row and leave it unchanged when a theme color is missing.

## Search programs

[`ToolsManager`](../crates/maestro-tools/src/search/tools_manager.rs) is native-only.
Its constructor takes a binary-directory string and an optional replacement HTTP
transport. `get_tool_path` checks that directory first, then probes `fd`, `fdfind`
(in that order) or `rg` on `PATH` with `--version`. A candidate passing
[`Path::exists`](https://doc.rust-lang.org/std/path/struct.Path.html#method.exists)
wins even when it is not executable; a started probe counts regardless of exit status.
Dropping discovery kills its running probe.

`ensure_tool` returns an existing result without advice. Otherwise it reads
`MAESTRO_OFFLINE` at the call through the [environment flag helper](text.md);
an enabled flag skips acquisition.
Android instead receives `pkg install fd` or `pkg install ripgrep` advice.

Other hosts request the latest release from `sharkdp/fd` or `BurntSushi/ripgrep`
on GitHub. Release lookup has a 10-second deadline and archive download a
120-second deadline, each including the body. Supported assets are macOS/Linux
`.tar.gz` archives extracted by system `tar`, and Windows `.zip` archives
extracted in process. Each install stages inside a separately created directory,
leaving an archive-named user file in the binary directory untouched. Unix
permissions become executable before rename publication; Windows skips chmod.
Cleanup is attempted when staging is dropped, without replacing the primary result.

An acquisition failure returns `None` and writes `Failed to download <name>: <detail>`
when an output is supplied. Optional output uses the caller's writer and colour flag:
dim progress, yellow advice/failure, or plain lines. No output means silence; writer
errors do not change availability. Find uses silent acquisition; content search
execution is a separate operation.

## Find

[`create_find_tool` and `create_find_tool_definition`](../crates/maestro-tools/src/search/find/mod.rs)
take a working directory, `FindToolOptions` and an `Arc<ToolsManager>`. Construction
neither resolves paths nor acquires a program. Both factories are native-only.

`pattern` is a required glob string, for example `*.ts` or `src/**/*.spec.ts`.
`path` defaults to `.` when absent or empty; execution delegates authored path
resolution to [`resolve_to_cwd`](../crates/maestro-tools/src/path_utils.rs).
`limit` defaults to 1000 and is forwarded as a number without rounding.

With supplied `FindOperations`, execution checks `exists` before calling `glob`
with the resolved root, pattern, original limit and the ordered ignore patterns
`**/node_modules/**` and `**/.git/**`. These operations replace native work,
including acquisition. Their returned vector is not sliced or sorted. A nonempty
vector whose length reaches the limit reports that limit, including zero or a
negative supplied limit. A missing root fails with `Path not found: <path>`;
operation failures retain their messages.

Otherwise execution acquires fd silently through the [search program owner](#search-programs).
The program handles glob syntax and scoped ignore files. It includes hidden user
files and unignored dependencies, excludes `.git`, and receives zero as unlimited.
Negative and fractional limits are left for fd to reject. Patterns containing `/`
use full-path mode, with `**/` prefixed unless already rooted or prefixed; `--`
separates patterns from options. NUL-delimited output keeps filename whitespace,
embedded newlines and directory markers. Native nonzero exits with records return
those records; without records they fail with trimmed stderr or an exit-code message.

Relative returned paths are anchored at the search root; absolute results are
projected against it through [lexical paths](../crates/maestro-path/src/lib.rs),
including Windows drive context. Returned directory separators are retained as `/`;
a backslash remains literal on POSIX. The result keeps producer order and duplicates. No matches returns `No files found matching pattern` without
details. Nonempty bodies join with LF and use the shared
[head selection](tools-read.md#limits-and-selection), with no separate line cap.
Count and byte notices follow after a blank line, count first. Native positive
limits add doubled-limit/refine advice; native zero reports no count limit.
Details are absent unless count or byte truncation is reported.

A pre-aborted signal avoids path resolution. Abort during supplied effects or
acquisition settles `Operation aborted`. Abort during native waiting requests
ordinary child termination and returns without waiting for exit; no escalation
or tool-owned deadline is added.

The call renderer shows `find`, the pattern, ` in <path>` and a present limit.
Result rendering shares the [listing preview](#rendering), using `results` for its
count unit. Result options alone select expansion; successful rendering reuses the
row's previous `Text`, and a rendering failure leaves its text unchanged.

# Session analysis reports

Four developer reports read the journals Maestro already writes under the home
session root. Two print totals for one project, one writes transcript files with
an optional analysis run, and one writes an interactive tool report. They are
checkout-run example programs: they read recorded journals and write nothing
except their own output. No background collection or telemetry is involved, and
the programs make no network request of their own. Two things reach the network
beyond them: the child runs `--analyze` starts use the configured model provider,
and the tool report page, once opened, loads its two pinned chart and style
scripts from their source URLs.

Every program accepts its arguments after `--` and decodes them lossily, so a
non-UTF-8 argument reaches the report as replacement text instead of aborting
the process.

## Cost totals

```sh
cargo run --locked -p maestro-app --example cost_report -- -d /work/project -n 7
```

`-d`/`--dir` selects the project and `-n`/`--days` the window; both are
required. The day count is read as an integer prefix, so `7suffix` means 7, an
empty value counts as absent, and `0` or text fails with the same diagnostic as
a missing argument. `-h`/`--help` prints the usage text before anything is read.
Unknown arguments are ignored.

The report keeps the directory under the home session root,
`~/.maestro/agent/sessions/--<project>--`, where `<project>` is the authored path
with one leading separator removed and every other `/`, `\` or `:` replaced by a
dash; the path is not normalized or made absolute. A missing directory fails
with the path it looked for; an empty or fully filtered directory reports that
no sessions were found.

Each journal whose own filename instant is at or after the local midnight `-n`
days before today contributes its assistant messages that report a cost. The cost
report alone takes its root from the `HOME` variable; the other three use the
account-aware home directory, so they still work where `HOME` is unset. Days
are UTC calendar days of the message instant, providers keep their recorded
names including ones that collide with inherited object properties, and every
amount is spelled with four fractional digits, except that one of 1e21 or more
keeps the shortest scientific spelling, such as `1e+21`. Days, providers and the
grand total are listed in UTF-16 order, and amounts the record omits count as zero.

## Usage totals

```sh
cargo run --locked -p maestro-app --example usage_report -- --cwd /work/project --days 7
```

`-d`, `--dir` and `--cwd` select the project; `--days`/`-n` select the local
calendar days, defaulting to seven, and `--sessions-base` overrides the session
root. Relative paths resolve against the working directory. An empty option
value counts as absent, which leaves that option's default in place. A missing
value keeps the default as well. A day count that is not a positive integer is
rejected.

The window starts at local midnight `days - 1` dates before today and ends at
local midnight tomorrow, so it follows daylight-saving changes rather than a
fixed number of hours. A message's own timestamp wins over the entry's, and a
present `null` timestamp is epoch zero rather than a fallback. Reported token
and cost members are summed as recorded; `totalTokens` and `cost.total` are
never recomputed from their components, and a missing total counts as zero. Session counts are distinct journal
names.

Counts are rounded to the nearest integer, a half toward positive infinity, and
grouped in en-US spelling; a rounded negative zero stays visible as `-0`. Costs
keep four fractional digits, with the same scientific spelling from 1e21 up. Days
and providers are listed in UTF-16 order.

## Transcript files

```sh
cargo run --locked -p maestro-app --example transcript_report -- --output ./session-transcripts --analyze /work/project
```

`--output` selects the output directory, defaulting to `./session-transcripts`,
and the first value wins. Any argument that is not a flag and not an output
value selects the project, defaulting to the working directory; `--help` is
ignored and `-h` is an ordinary path. The output directory is created before the
session directory is looked up. A missing session directory and an empty one
each report their own outcome and exit one.

Sessions are named in UTF-16 order and each becomes one wrapper holding its
stored user and assistant messages in file order, split by `---`. Message text
keeps string content as authored and joins the nonempty blocks of type `text`
with a line feed; blocks of any other type are ignored, even when they carry a
`text` member, and a message whose text is only whitespace is dropped.

Wrappers are packed into `session-transcripts-NNN.txt` files up to 100,000 UTF-16
units, separated by a blank line. A wrapper larger than that goes to its own
numbered file and is reported as oversized.

`--analyze` runs one child per written file and then one aggregating child in
the output directory, with the `read` and `write` tools only. Each child receives
the paths of any existing instruction files, the transcript or summary paths and
the line count, and its JSON event stream is rendered as text. Analysis outcomes
are reported but do not change the command's successful exit; a rejected
operation is printed and the command still exits zero.

## Tool report

```sh
cargo run --locked -p maestro-app --example tool_report -- --sessions-dir ./sessions --output ./tool-stats.html
```

`--sessions-dir` defaults to the home session root and `--output`/`-o` to
`maestro-tool-stats.html` in the temporary directory. Every `.jsonl` file
beneath the chosen directory is read, directories included, in file-name order.
The page is written before the platform viewer is started, and it names no other
program.

Tool calls, results and errors are counted per tool, the reported result text
size is estimated as a quarter of its UTF-16 units rounded up, and each result's
estimate is also attributed to the bash command of the call it answers, keyed by
the first command word and an optional non-option second word. Environment
assignments and one `sudo` are stripped only while a command word remains, so a
command of nothing but assignments keeps its final word and a bare `sudo`
stays `sudo`. A result answering an empty command is counted for its tool only.
At most fifty bash rows are listed.

The generated page keeps the report layout, selectors, charts and histogram
canvases. Its colors and font stacks come from the shared brand pack as CSS
custom properties, and the two chart and style libraries stay the script source
URLs the reference writes, so nothing is bundled. The charts paint those roles
rather than library defaults: bar fills take the accent, tick and legend text the
muted color in the body font, grid lines the border color, and a table row's top
border the border color. A committed fixture page holds the generated bytes, and
the browser job selects a row, checks that the payload stays text and reads the
painted pixels back: the two library scripts come from a digest-checked cache, and
any other request that would leave the machine fails the run.

## Reading the journals

Every report reads each selected journal once and aggregates it in one pass. Cost
and transcript parsing remove outer whitespace from the whole file before
splitting lines, while usage and tool parsing keep the authored lines so a
leading byte-order mark reaches the parser and the line is counted as malformed.
Blank lines are skipped, never counted as malformed, and entries of another type
or role are ignored rather than rejected.

## Timestamps

Report days come from the session library's local-time resolver, which is also
where stored timestamps are read. A local time that occurs twice resolves to the
earlier instant, and a local time skipped by a daylight-saving change is read with
the offset in force before the change, so it lands past the gap by the size of the
change instead of being rejected.

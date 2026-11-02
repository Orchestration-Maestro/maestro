# Tool and terminal additions

This specification defines thirteen owner-approved additions to Maestro's tools and
terminal. The foundation changes are listed in
[Amendment to the foundation specification](#amendment-to-the-foundation-specification),
and the terms in [Glossary](#glossary) are also defined in [the glossary](../../CONTEXT.md).

## Purpose and authority

The [foundation specification](maestro-port.md) defines Maestro's baseline. Every
addition here is an owner-approved difference from that baseline, ported from one of
two references:

- **The addition reference** is a Rust and TypeScript agent built on the same
  baseline. Its Rust crates and their TypeScript callers are the source of each
  addition's behaviour, values and tests.
- **The format reference** is the upstream implementation of the `apply_patch` edit
  format (Rust, Apache-2.0). It decides how patches are parsed, matched and
  reported.

Each reference file maps to a Rust module with the reference's exported names in Rust
casing. The inside is idiomatic Rust; it is never a transliteration. The reference's
tests are ported red first under Maestro names. Reference defects are fixed and
listed once in [Corrected reference defects](#corrected-reference-defects).

Every addition keeps the foundation behaviour unless a setting turns it on, with
three exceptions:

- the terminal output backlog (A1) replaces a runtime mechanic and changes no byte
  that reaches the terminal;
- SIXEL images (A11) and Mermaid diagrams (A13) only change what a terminal shows,
  and each can be turned off. Neither changes what the model receives.

No addition uses `unsafe` code, a patched fork or a vendored copy of a library.

Scheduling: A1 starts at once. A2 to A13 start after the foundation port is complete.

## Problem statement

Maestro reproduces its baseline exactly, including weaknesses its users feel every
day:

- A slow or busy terminal freezes the whole interface: while a large frame drains,
  keys are not read and nothing else happens.
- Models trained on the `apply_patch` edit format must use a different edit tool.
- Verbose command output (builds, test runs, package managers) fills the context
  window.
- No tool searches or rewrites code by structure, and reads return whole files where
  a structural summary would do.
- The read tool returns PDF files as undecoded bytes.
- Parallel subagents share one checkout and overwrite each other's work, and a single
  `git stash` or `git reset --hard` can discard another agent's changes.
- Programs that need a terminal (password prompts, interactive installers) cannot run
  under the bash tool.
- Instructions in AGENTS.md files below the working directory are never mentioned to
  the model.
- Notebooks are read and edited as raw JSON, outputs and images included.
- Terminals that only support SIXEL show no images.
- Long runs stop when the machine goes to sleep.
- Mermaid diagrams in answers appear as source text.

## Solution

Thirteen additions, each owned by the crate that already owns its neighbours:

| # | Addition | Owner | Default |
|---|---|---|---|
| A1 | Terminal output backlog | `maestro-tui-crossterm`, `maestro-tui` | always (no visible change) |
| A2 | `apply_patch` edit format | `maestro-tools`, `maestro-app`; grammar form in `maestro-models` | off (`edit.mode`) |
| A3 | Output minimizer | `maestro-tools`; new `maestro-shell` | off (`shellMinimizer.enabled`) |
| A4 | Syntax trees: tools, summaries, block context | new `maestro-syntax`, `maestro-tools` | opt-in tools; summaries off (`read.summarize.enabled`) |
| A5 | PDF reads | `maestro-tools` | off (`pdfAsText`) |
| A6 | Isolated subagent runs | `maestro-app` host action, `maestro-daemon` worktrees | off (`task.isolation.enabled`) |
| A7 | Git guard | new package `maestro-git-guard`, `maestro-shell` | when installed |
| A8 | Interactive commands | `maestro-tools`, `maestro-process`, `maestro-terminal` | off (`bash.pty`) |
| A9 | Nested AGENTS.md listing | `maestro-resources`, `maestro-app` | off (`nestedAgentsFiles`) |
| A10 | Notebooks as cells | `maestro-tools` | off (`notebooksAsText`) |
| A11 | SIXEL images | `maestro-tui`, `maestro-tui-crossterm` | on (`terminal.showImages`) |
| A12 | Keep the machine awake | `maestro-app`, `maestro-process` | off (`power.sleepPrevention`) |
| A13 | Mermaid diagrams | new `maestro-diagram`, `maestro-theme`, `maestro-tui` | on (`tui.renderMermaid`) |

HTML to Markdown conversion, approved with PDF reads, is delivered by the web access
package's shared HTML library in [Maestro packages](packages.md); it has one owner and
no second converter here.

## User stories

1. As a user on a slow terminal, I want typing and cancelling to keep working while a large frame drains, so that the interface never freezes.
2. As a user, I want the bytes my terminal receives, and their order, to stay exactly what they were, so that nothing renders differently.
3. As a user on a slow terminal, I want intermediate frames skipped while output is backed up, so that the screen catches up to the latest state instead of replaying every frame.
4. As a user quitting while output is backed up, I want every queued byte written before the terminal modes are restored, so that my shell is left clean.
5. As a user of models trained on the `apply_patch` format, I want the edit tool offered in that format, so that the model edits files the way it was trained to.
6. As a user switching between models, I want each model to get the edit format configured for it, so that other models keep the standard edit tool.
7. As a user, I want a patch that fails anywhere to change no file, so that a half-applied patch never leaves my tree inconsistent.
8. As a user, I want patch results reported in the format the model expects, with the same diff view as other edits, so that the model and I see what changed.
9. As a user of OpenAI models that support grammar-constrained tools, I want `apply_patch` offered with its grammar, so that the model's patches are always well formed.
10. As a user, I want verbose command output reduced before the model reads it, so that the context window holds more useful work.
11. As a user, I want the full output kept in a file and named in the result, so that nothing is lost when the reduction hides a detail.
12. As a user, I want reduction to apply only to a single simple command it recognises, and every pipeline or compound command passed through unchanged, so that a reduction never misrepresents output.
13. As a package author, I want output filters to be data files, so that I can add a filter for my program without code.
14. As a user, I want to restrict which filters run, so that I can turn off a filter I distrust.
15. As a user, I want a tool that finds code by structure across files, so that the model can locate patterns text search cannot.
16. As a user, I want a tool that rewrites code by structure, previewing changes by default, so that broad refactors are safe to review.
17. As a user, I want an unsupported language reported as an error, never a crash, so that a bad request does not stop the session.
18. As a user, I want reads of large source files to return a structural summary when no range is requested, so that the model sees the shape of a file cheaply.
19. As a user, I want a read of a line range to show the enclosing block headers, so that the model knows which function the lines belong to.
20. As a user, I want the read tool to read PDF files as text with page numbers, so that the model can use documents.
21. As a user, I want line ranges to apply to the converted PDF text, so that the model can page through a long document.
22. As a user running subagents in parallel, I want each isolated subagent to work in its own worktree that starts from my uncommitted work, so that they never overwrite each other.
23. As a user, I want an isolated subagent's own changes applied back as a patch or merged from a branch, so that I choose how work comes home.
24. As a user, I want worktrees created for isolated runs cleaned up afterwards, and kept with their paths when something fails, so that nothing accumulates and nothing is lost.
25. As a user sharing one checkout between agents, I want commands that discard or move shared work refused, so that one agent cannot destroy another's changes.
26. As a user in a merge or rebase conflict, I want conflict-side checkouts still allowed, so that the guard does not block conflict resolution.
27. As a user, I want a command that needs a terminal to run in an overlay I can type into, so that password prompts and interactive programs work.
28. As a user in print or RPC mode, I want an interactive request to run without a terminal and say so, so that non-interactive runs never hang.
29. As a user, I want AGENTS.md files in subdirectories listed in the system prompt, so that the model reads them before changing those directories.
30. As a user, I want that list bounded and turned off by `--no-context-files`, so that the prompt stays small and the existing switch keeps working.
31. As a user, I want notebooks read and edited as cell text, so that the model sees code instead of JSON and outputs.
32. As a user, I want an edited notebook written back as valid notebook JSON with its outputs preserved, so that the notebook still opens.
33. As a user on a SIXEL-only terminal, I want images shown, so that I see screenshots and figures.
34. As a user, I want the machine kept awake while an agent works, at the level I choose, so that long runs are not interrupted by sleep.
35. As a user, I want the inhibition released when the work ends or Maestro exits, so that my machine sleeps normally afterwards.
36. As a user, I want Mermaid blocks in answers drawn as diagrams in the terminal, so that I can read them without another tool.
37. As a user, I want a diagram that cannot be drawn shown as its source, so that nothing is lost.
38. As an SDK author, I want every addition reachable through the same settings, tool registry and extension host as the foundation, so that embedding programs and components can use them.
39. As a maintainer, I want every addition free of `unsafe` code and patched libraries, so that the safety rules have no exception.

## Behaviour contract

### A1 — Terminal output backlog

1. Every write the native terminal makes (frames, cursor, title, progress, modes)
   enters one ordered output queue and returns at once. One writer task on the
   terminal's existing reactor drains the queue when the descriptor is writable. No
   thread is added.
2. Bytes reach the terminal in exactly the order they were written; their content is
   unchanged.
3. The terminal reports its queued byte count. The toolkit defers a frame while more
   than 256 KiB are queued and retries after 10 ms; the deferred frame draws the
   latest state, keeping any forced or clearing intent. Values from the addition
   reference. A final frame claimed just before the terminal stops is not deferred:
   it is one bounded frame and the stop drains it with the rest of the queue.
4. Stopping and dropping the terminal write every queued byte before restoring modes,
   as today.
5. A write error is recorded as the terminal's first background failure, reported by
   the next stop, and the queue is dropped.
6. A descriptor the reactor refuses (a regular file, `/dev/null`) is written
   synchronously, as reads are today.

### A2 — `apply_patch` edit format

1. A tool named `apply_patch` takes one string argument, `input`, holding a patch:
   `*** Begin Patch`, file sections, `*** End Patch`. Sections are `*** Add File:`,
   `*** Delete File:` and `*** Update File:` with an optional `*** Move to:`, `@@`
   context lines and hunk lines, and an optional `*** End of File` marker. The
   grammar and the lenient parsing (whitespace around markers, a missing final
   newline) follow the format reference.
2. Hunks are located with the format reference's four passes, in order: exact, then
   ignoring trailing whitespace, then ignoring surrounding whitespace, then after
   Unicode punctuation and space normalisation. Each hunk binds to its first match at
   or after the previous hunk. No similarity-ratio or edit-distance matching is used.
3. Adding an existing file and moving onto an existing file overwrite, as the format
   reference does. Deleting a missing file, updating a missing file and deleting a
   directory fail.
4. Line endings are preserved per file, including mixed endings.
5. All sections are staged first. If any section fails, no file changes and the error
   names the section, the file and the reason.
6. Paths resolve with the foundation's path resolution. Writes go through the
   per-file mutation queue shared with `edit` and `write`.
7. The result reports the changed files in the format reference's summary form. The
   rendered diff uses the foundation's edit diff.
8. Selection: `edit.mode` (`replace` by default, or `apply_patch`) and
   `edit.modelVariants` (case-insensitive model substring to mode; first match wins)
   decide whether the active tool set has `edit` or `apply_patch`. A model change
   re-evaluates the choice and rebuilds the tool set and system prompt when it
   changes.
9. Grammar form: for OpenAI Responses models whose catalog entry marks
   `applyPatchToolType: "freeform"`, `apply_patch` is sent as a custom tool
   constrained by the format's Lark grammar. Its calls arrive as custom tool calls
   with raw text input, and results replay as custom tool call outputs. Models
   without the mark receive the function form.

### A3 — Output minimizer

1. With `shellMinimizer.enabled`, after a bash command ends, its captured output is
   reduced when the whole command is one simple command whose program has a filter.
   Anything else passes through unchanged: unquoted `|`, `&&`, `||`, `;`, `&`,
   subshells, compound commands and any input the scanner cannot prove simple.
2. Filters are data first. Built-in filter definitions ship as TOML files with their
   own test cases. A user filter file named by `shellMinimizer.settingsPath` adds
   definitions searched before the built-in ones. Code filters handle programs whose
   output needs a state machine.
3. `shellMinimizer.only` and `shellMinimizer.except` restrict filters by name.
   `shellMinimizer.maxCaptureBytes` (4 MiB) bounds the captured text a filter reads.
   Larger output passes through. Values from the addition reference.
4. Short output (below the reference's minimum) is never reduced. A filter failure
   passes the output through.
5. When output is reduced, the full output is saved to the bash tool's existing
   full-output file. The model sees the reduced text followed by a notice naming
   that file.
6. `shellMinimizer.sourceOutlineLevel` (`default` or `aggressive`) selects the
   outline used when a source file is printed by a listing command.

### A4 — Syntax trees

Tools:

1. `ast_grep` finds structural pattern matches in files or paths. `ast_edit` applies
   structural rewrites and previews them by default. Arguments, strictness levels,
   metavariables, limits and result text follow the addition reference.
2. Both tools are opt-in through the tool list, like `grep`, `find` and `ls`.
3. Languages come from a fixed set of linked grammar crates chosen at build time.
   Nothing is downloaded at run time. An unsupported or unknown language is a typed
   error naming it.
4. Overlapping rewrites are rejected. Edits go through the per-file mutation queue.

Reads:

5. With `read.summarize.enabled`, a read of a supported source file without an offset
   or limit returns a structural summary. The `read.summarize.*` thresholds decide
   when, with the addition reference's defaults.
6. With the same setting, a ranged read whose window cuts through blocks shows the
   enclosing block header lines. Without a parse, the lexical bracket fallback of the
   addition reference applies.
7. Offset, limit and truncation apply as for any read.

### A5 — PDF reads

1. With `pdfAsText`, reading a `.pdf` file returns its text page by page with page
   numbers, in the page-text format browser attachments already use, extracted by the
   PDF engine the browser application already uses.
2. Offset, limit and truncation apply to the converted text.
3. Pages without extractable text are named in a closing notice. An unreadable PDF
   returns the read tool's conversion failure text, with the reason.
4. One PDF text owner serves the browser attachments and the read tool.

### A6 — Isolated subagent runs

1. With `task.isolation.enabled`, a subagent task may ask for isolation. The run then
   happens in a new detached git worktree of the parent repository, created by the
   workspace daemon's worktree owner.
2. The worktree starts from the parent's state, including its staged, unstaged and
   untracked work and its nested repositories. A working tree whose uncommitted
   content exceeds the addition reference's budget is refused before anything is
   created.
3. After the run, only the task's own changes are captured, as one patch per
   repository; the parent's carried work is subtracted.
4. `task.isolation.merge` decides how changes come home. `patch` (default) applies the
   combined patches to the parent checkout, with a three-way fallback. `branch`
   commits each task on its own branch, keeping the agent's commit messages, and
   merges with `--no-ff`. `task.isolation.apply` set to false leaves the changes
   captured and names them.
5. Worktrees and task branches are removed after a successful finish. A failed apply
   or merge keeps them and reports where they are. A worktree whose owning process is
   still alive is never cleaned.
6. The lifecycle is one application operation offered to extension components as a
   host action. The subagent example component is its first caller; any subagent
   package reuses the same action.
7. Only git worktrees through the `git` program are used.

### A7 — Git guard

1. The `maestro-git-guard` extension package registers a `tool_call` handler. Before
   bash runs, it blocks commands that discard or move work in a shared checkout:
   - `stash`, except `stash list` and `stash show`;
   - `reset --hard`, and `reset` to a commit other than HEAD;
   - `checkout`, `switch` and working-tree `restore`, unless a merge, rebase,
     cherry-pick or revert is in progress. Even then, forced forms and whole-tree
     paths stay blocked.
2. Index-only commands (`reset <path>`, `restore --staged`, a bare `checkout`) run.
3. Every `git` invocation in the command line is judged, across `&&`, `||`, `;`, `|`
   and grouping, with git's own global options read and a literal `cd` followed.
   A command that points git at another repository through options or location
   variables is blocked when its exemption cannot be verified.
4. The block reason is the addition reference's text for the rule. Git started by an
   absolute path, through another program or from an expansion is not seen; the guard
   is not a sandbox.

### A8 — Interactive commands

1. With `bash.pty`, the bash tool accepts `pty: true`. In the interactive frontend the
   command runs under a pseudo-terminal in an overlay titled "Console". The screen is
   emulated; the user types into the program, the overlay follows resizes, and Esc
   kills the program.
2. The model receives the program's output, with line endings normalised, through the
   bash tool's usual limits and full-output file.
3. Without an interactive frontend (print, JSON, RPC), the command runs without a
   terminal and the result says so in the addition reference's words.
4. The child gets the bash environment without its non-interactive guards (`CI`,
   `GIT_EDITOR`, `GPG_TTY`, `NO_COLOR`) and with `TERM=xterm-256color`, as in the
   addition reference.
5. A `pty` call never runs beside another tool call.

### A9 — Nested AGENTS.md listing

1. With `nestedAgentsFiles`, the system prompt gains a block after the project
   context. It lists the AGENTS.md files in directories 1 to 4 levels below the
   working directory, as sorted relative paths, at most 200, and tells the model to
   read the relevant ones before changing those directories. File contents are not
   loaded.
2. Directories are pruned by the project's ignore rules, by hidden names and by the
   addition reference's list of build and dependency directory names. An AGENTS.md
   file is found even when an ignore rule names the file itself.
3. `--no-context-files` drops the block.

### A10 — Notebooks as cells

1. With `notebooksAsText`, reading an `.ipynb` file returns one text block per cell,
   headed `# %% [<type>] cell:<n>`, with the source and no outputs. Offset, limit and
   truncation apply to that text.
2. `edit` and `apply_patch` on an `.ipynb` file edit that text. The result is encoded
   back into the notebook JSON with outputs, metadata and key order kept, in the
   notebook's own serialisation (one-space indentation). A new notebook starts as an
   empty nbformat 4.5 document.
3. Malformed notebooks return the notebook error texts of the addition reference.

### A11 — SIXEL images

1. When the environment names no image protocol, the toolkit asks the terminal once
   at start for its graphics geometry and primary device attributes. Attribute 4, or
   a successful geometry reply, enables SIXEL; inside tmux, only when tmux reports
   that its client renders SIXEL. Replies are consumed and never reach input.
2. Images are scaled to the existing cell limits, composited over the terminal
   background and encoded as SIXEL.
3. On terminals detected by the existing protocols, or with `terminal.showImages`
   off, no query is sent and nothing changes.

### A12 — Keep the machine awake

1. `power.sleepPrevention` is `off` (default), `idle`, `display` or `system`. Each
   level adds the lower levels' effect.
2. While a prompt is in flight, Maestro holds a sleep inhibitor through a system
   command bound to its own process: `systemd-inhibit` on Linux and `caffeinate` on
   macOS, with the addition reference's flags for each level. The inhibitor ends when
   no prompt is in flight, on abort, when the session ends and when Maestro exits.
3. A missing inhibitor command disables the feature for the process with one debug
   log line and no warning.

### A13 — Mermaid diagrams

1. A fenced code block whose info string is `mermaid` is drawn as a text diagram:
   flowcharts, state, sequence, class, entity-relationship and XY charts. The drawing
   chooses the orientation that fits the width and clips rows to it.
2. Source that does not parse, or a diagram type outside the list, renders as the
   ordinary code block.
3. `tui.renderMermaid` (default on) turns diagrams off. The system prompt does not
   change.

## Implementation decisions

- **Terminal backlog.** The native terminal's input source type becomes one
  descriptor type, readable or writable, over the reactor or a plain file. The output
  queue lives in the terminal's shared state with one writer task per started
  generation. The terminal trait gains a provided method reporting queued bytes,
  zero by default, so every other terminal is unchanged. The toolkit's render
  schedule consults it before drawing.
- **`apply_patch`.** One module in `maestro-tools` holds parsing, hunk seeking and
  staged application. It reuses the edit diff, path resolution and file mutation
  queue. Tool selection by setting and model is an application operation in session
  services. The grammar form is an OpenAI Responses adapter capability, keyed by a
  catalog field on the model record.
- **Shell command lines.** A new leaf crate, `maestro-shell`, splits shell command
  lines into simple commands and their words, and flags lines with syntax beyond
  them. It replaces the addition reference's shell parser library and is the one
  scanner for the minimizer and the git guard.
- **Minimizer.** One module in `maestro-tools` owns detection, the plan, the filter
  pipeline and the filter definitions as bundled data. Its only caller is the bash
  tool after a command ends. It is the single owner of output reduction for Maestro
  and its packages.
- **Syntax trees.** A new leaf crate, `maestro-syntax`, parses source code into syntax
  trees: languages, patterns, rewrites, summaries and block boundaries. Its only
  internal dependent is `maestro-tools`. It is not the theme's highlighter: the
  highlighter is lexical and the two share no logic.
- **PDF text.** The PDF text extraction moves from the browser application's
  attachment utilities into `maestro-tools`. The application keeps page rendering
  and calls the moved extraction. No crate or crate edge is added; the PDF engine
  dependency moves with the code.
- **Isolation.** Worktree creation, listing and removal belong to the workspace
  daemon's worktree owner; every other git command runs through `maestro-process`.
  The application owns the lifecycle and offers it to components as a host action
  through the extension host port, the canonical interface and the runtime adapter.
- **Git guard.** A package crate using the existing `tool_call` hook; no core change.
  It reuses `maestro-shell`, which the package crates may depend on, and reads the
  repository through the host's process capability.
- **Interactive commands.** The pseudo-terminal comes from `maestro-process`. The
  screen comes from `maestro-terminal`, the one emulator, through its styled rows and
  key encoder; `maestro-tools` gains that edge. The bash tool reaches the overlay
  through a small port that `maestro-app` supplies from the extension UI's custom
  overlay path when the session has a UI.
- **Nested AGENTS.md.** `maestro-resources` lists the files, pruned by
  `maestro-pattern`'s ignore rules. The resource loader carries the list. The system
  prompt options gain an optional list, omitted when empty, and the system prompt
  builder renders it.
- **Notebooks.** One codec module in `maestro-tools`, reached from `read`, `edit` and
  `apply_patch`.
- **SIXEL.** The toolkit's image capability detection and protocol encoding gain a
  SIXEL variant, and the native terminal consumes the device-attributes reply. The
  encoder is Maestro's own, over the approved `image` library.
- **Keep awake.** The application's prompt lifecycle holds one owned child process
  through `maestro-process`.
- **Mermaid.** A new crate, `maestro-diagram`, draws Mermaid diagrams as text, with
  display widths from `maestro-text`. The theme supplies the toolkit's Markdown
  component with a Mermaid resolver, as it supplies highlighting; the toolkit does not
  depend on the diagram crate.
- **Settings.** Every setting above is read through the settings manager's typed
  accessors. Nested keys use the existing nested-key access.

## Testing decisions

- Test external behaviour at the owning public interface:
  - the terminal through a pseudo-terminal pair whose reader is paused and resumed;
  - tools through their tool definitions with controlled file systems and processes;
  - provider adapters through recorded request and response fixtures;
  - the system prompt through the prompt builder;
  - extensions and host actions through a real component on the host path;
  - toolkit rendering through the terminal test harness;
  - git behaviour against real temporary repositories.
- Port every reference test of the scope red first under Maestro names, including
  data cases:
  - the filter definitions' embedded cases and the minimizer fixtures;
  - the format reference's scenario directories;
  - the notebook golden files;
  - the Mermaid fixture set.
- Where Maestro deliberately differs (atomic patches, typed errors instead of panics),
  the ported test asserts Maestro's behaviour and the report lists the difference.
- Settings defaults are tested: with no setting, the tool list, read output, bash
  output, terminal bytes and system prompt equal the foundation behaviour.
- Prior art: `terminal_output.rs` and `terminal_lifecycle.rs` (native terminal), the
  edit and bash tool tests, the read tool tests, provider fixture tests, resource
  loader tests and Markdown component tests.

## Owner-approved additions

Each item is an owner-approved difference with a reference source in the addition or
format reference:

1. The terminal output queue and frame deferral (A1).
2. The `apply_patch` tool, its grammar form, `edit.mode`, `edit.modelVariants` and the
   `applyPatchToolType` catalog field (A2).
3. The output minimizer, `maestro-shell` and the `shellMinimizer.*` settings (A3).
4. The `ast_grep` and `ast_edit` tools, read summaries, block context and the
   `read.summarize.*` settings (A4).
5. PDF reads and `pdfAsText` (A5). HTML to Markdown is approved with them and
   delivered by the packages specification.
6. Isolated subagent runs, the isolation host action and the `task.isolation.*`
   settings (A6).
7. The git guard package (A7).
8. Interactive commands and `bash.pty` (A8).
9. Nested AGENTS.md listing and `nestedAgentsFiles` (A9).
10. Notebooks as cells and `notebooksAsText` (A10).
11. SIXEL images and their start-up queries (A11).
12. Sleep prevention and `power.sleepPrevention` (A12).
13. Mermaid diagrams and `tui.renderMermaid` (A13).

`pdfAsText`, `bash.pty`, `nestedAgentsFiles` and `notebooksAsText` are Maestro's
names. The addition reference has no switch for these behaviours; Maestro adds one so
that each stays off until the user turns it on. `power.sleepPrevention` keeps the
addition reference's name with `off` as its default.

## Corrected reference defects

1. The format reference keeps earlier files changed when a later section fails.
   Maestro stages every section and changes nothing on failure.
2. The addition reference panics when a grammar is not installed. Maestro returns a
   typed error naming the language.
3. The addition reference's edit layer silently replaces a missing path with the
   unique file sharing its suffix. Maestro resolves paths literally, as the foundation
   does.
4. The addition reference's diffs add syntax-tree block rows to every edit. Maestro
   keeps the foundation's line-context diff.
5. The addition reference's scan cache serves results up to one second old. Maestro
   has no scan cache.

## Runtime mechanics not copied

- The addition reference's writer thread, its locks, atomics and raw descriptor
  calls, and its UTF-16 transcoding arena. Maestro writes from its existing reactor.
- The addition reference's stall watchdog that declares a wedged terminal
  disconnected. A wedged terminal blocks the final drain on stop, as the foundation
  does.
- Grammar downloads, a second WebAssembly runtime for grammars, parse-store budgets
  and grammar directory setters.
- The JavaScript bridge layer, asynchronous variants and encodes that exist only to
  leave the JavaScript thread, drain logic that waits on JavaScript callbacks, and
  promise scheduling.
- The in-process git library behind the addition reference's isolation; Maestro runs
  the `git` program.
- The format reference's shell heredoc invocation parsing, its standalone executable
  and its symlink no-follow option.
- The addition reference's embedded shell, chain-mode segment capture and the forked
  shell parser it needs.
- Platform power-management calls; the platforms' inhibitor commands replace them.

## Out of scope

- The addition reference's own edit formats (hashline, sloppy, its unified `patch`
  mode) and its similarity-ratio matching ladder.
- Copy-on-write, overlay and projected-filesystem isolation backends; Jujutsu;
  session worktrees started at launch; generated commit messages for isolated runs.
- A workspace tree dump in the system prompt, and the prompt hint that diagrams are
  drawn.
- SIXEL decoding of program output for the model, and the environment variable that
  forces an image protocol.
- The desktop screensaver inhibitor on Linux and sleep prevention on Windows (the
  Windows phase).
- In-process search, glob and fuzzy finding, a shared scan cache, an embedded shell
  and its builtins, gitoxide and Jujutsu, voice, computer use, token counting,
  image-based compaction and vector kernels.
- HTML to Markdown conversion and network fetching: they belong to the web access
  package.

## Glossary

Add to `CONTEXT.md`:

- **Output backlog:** Bytes the native terminal has queued and not yet written.
- **Patch envelope:** The `apply_patch` text: `*** Begin Patch`, file sections with
  hunks, `*** End Patch`.
- **Hunk seek:** Locating one patch hunk in a file with the four matching passes.
- **Simple command:** One program and its words in a shell command line, between the
  list and pipe operators.
- **Output minimizer:** The bash tool's reduction of a finished single simple
  command's output for the model; the full output stays in a file.
- **Output filter:** One data or code rule that reduces one program's output.
- **Syntax tree:** A parse of source code by a linked grammar; distinct from lexical
  highlighting.
- **Structural pattern:** Code with metavariables matched against syntax trees.
- **Code summary:** The structural outline a read returns instead of a whole file.
- **Isolated run:** A subagent run in its own worktree whose own changes are captured
  and then applied, merged or kept.
- **Git guard:** The extension that blocks git commands which discard or move shared
  work.
- **Interactive command:** A bash command run under a pseudo-terminal in an overlay
  the user types into.
- **Nested AGENTS.md:** An AGENTS.md file below the working directory, listed by path
  in the system prompt.
- **Notebook text:** The cell-by-cell text projection of an `.ipynb` file.
- **Sleep inhibitor:** The system lock Maestro holds while a prompt is in flight.
- **Text diagram:** A Mermaid diagram drawn with characters.

## Amendment to the foundation specification

In `docs/specs/maestro-port.md`:

1. Completion and explicit later scope: add "Owner-approved tool and terminal
   additions are specified in [tool and terminal additions](tool-additions.md)."
   This lands with this specification.
2. Crate table, as the first commit of the ticket that creates each crate:
   - `maestro-shell` | split shell command lines into simple commands | — | below 0,
     with `maestro-tools` gaining `maestro-shell` (A3);
   - `maestro-syntax` | parse source code into syntax trees | — | below 0, with
     `maestro-tools` gaining `maestro-syntax` (A4);
   - `maestro-diagram` | draw Mermaid diagrams as text | `maestro-text` | below 0,
     with `maestro-theme` gaining `maestro-diagram` (A13).
3. Edges between existing crates, as the first commit of their ticket:
   - `maestro-tools` gains `maestro-terminal` (A8);
   - package crates may depend on `maestro-shell`, and the package crate
     `maestro-git-guard` is added (A7).

## Delivery

| Ticket | Delivers | Blocked by |
|---|---|---|
| T01 | A1 | — |
| T02 | A2 function form and selection | foundation session services |
| T03 | A2 grammar form | T02 |
| T04 | A3 engine, data filters, `maestro-shell` | — |
| T05 | A3 code filters | T04 |
| T06 | A4 tools, `maestro-syntax` | — |
| T07 | A4 summaries and block context | T06 |
| T08 | A5 | PDF engine replacement |
| T10 | A6 | daemon worktrees, subagent component, component host capabilities |
| T11 | A7 | T04, package crate class |
| T12 | A8 | PTY children, emulator input, reads and key encoding, session lifecycle |
| T13 | A9 | application resources |
| T14 | A10 | T02 |
| T15 | A11 | — |
| T16 | A12 | session lifecycle |
| T17 | A13, `maestro-diagram` | Markdown parser |

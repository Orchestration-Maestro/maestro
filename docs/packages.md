# Package resources and acquisition

For installation commands, package authoring and resource selection, read
[install and author resource packages](package-guide.md).

`PackageManager::install` and `remove` acquire or delete the contents of one source
for the user scope or, with `Some(InstalledSourceScope::Project)`, the project scope;
`None` means user. The `*_and_persist` variants then edit settings. Source identity,
local path rules and settings storage belong to [configured package
sources](package-sources.md), `maestro-path` and `maestro-settings`.

The content-only [development skill package](packages/maestro-skills.md) uses the
same local-source resolution and conventional discovery.

## Sources

- **npm** (`npm:` prefix): the user scope runs `install -g <spec>` or
  `uninstall -g <name>`. The project scope works in `<project>/.maestro/npm`:
  install first prepares the root directory, then exclusively creates a missing
  `.gitignore` of `*` and `!.gitignore` and a missing `package.json` naming
  `maestro-extensions`, then runs
  `install <spec> --prefix <root>`; removal runs `uninstall <name> --prefix <root>`
  only when the root exists. The command is `npm` unless settings configure a
  nonempty `npmCommand` array, whose leading arguments come first (an empty array
  selects `npm`). An empty first entry fails with
  `Invalid npmCommand: first array entry must be a non-empty command`.
- **Git**: an existing target is fetched again: only the requested ref, or the
  upstream branch when none is requested. When the fetched commit differs from
  HEAD, the checkout is reset to it in place; a commit already current is left
  alone. Missing targets are prepared in
  an owned sibling directory and published only on success, below the scope's
  `git` root (with a `.gitignore` there). Dependencies are installed when there is a
  `package.json` (`install --omit=dev`, or plain `install` when a nonempty
  `npmCommand` is configured). The settings are read after the clone. Removal deletes the target and
  then empty ancestors strictly inside the Git root, comparing path components after
  resolving both against a working directory captured at most once, and only when a path needs it (per-drive directory lookups are separate); a failed ancestor removal ends the cleanup.
- **Local paths**: installation only checks that the path, resolved against the input
  base, exists (`Path does not exist: <path>` otherwise); removal does nothing.

Bootstrap creation retains existing entries, including dangling links, and accepts
an exclusive-create collision from a competing creator. Other directory/open/write
failures propagate as their original errors, leaving prior effects in place. Exclusive
creation prevents the losing writer from truncating a winner's file; it does not
publish a complete file atomically. Readers may observe an incomplete write, and a
failed write may leave a partial file. The filesystem adapter's `write_file` requires
a new pathname when `exclusive` is true; false retains create/truncate behavior.
Neither mode creates parent directories.

Installation does not consult the offline setting. [Available-update
checks](package-updates.md) consult it without acquiring contents. Configured
execution selects registrations before applying the offline return.

`DefaultPackageManager::with_offline` supplies the caller's startup selection.
`Some(true)` suppresses that remote work, `Some(false)` permits it, and `None` keeps
reading `MAESTRO_OFFLINE` whenever an operation consults offline mode. The flag
accepts `1`, or `true` and `yes` in any ASCII letter case, and is never trimmed.

## Resolved packages and pin integrity

`resolve` and `resolve_extension_sources` return `ResolvedPaths::packages` beside
resource lists. Each `ResolvedPackage` retains the authored source (including its
pin), a kind-prefixed identity (`npm:`, `git:` or `local:`), the lookup scope and
its installed path. Configured resolution keeps the project entry when user and
project entries name the same identity, even with different pins. Explicit
resolution retains source order and supports the temporary scope. Missing sources
that are skipped produce no descriptor. Empty packages and packages with all
resources filtered out still have descriptors; resource files are not needed to
find their roots.

`verify_pin(source, scope)` inspects an explicitly pinned Git source at its
managed user or project location. `None` selects user scope. `GitPinStatus` reports
the locally available expected commit, current HEAD and sorted relative changed
paths. Missing checkouts or unavailable refs have absent commits and are never
clean. Changes include the index, working tree, ignored files and other untracked
files. Inspection does not fetch or change contents.

`restore_pin(source, scope)` resets to the exact pinned commit and removes all
untracked contents, including nested repositories, even when HEAD already matches.
Read the status first to retain the paths for a restoration notice. Command
failure attempts to recover the previous HEAD selection. If local restoration is
unavailable or fails, online restoration prepares an ordinary installation in an
owned sibling before replacing the checkout. Preparation failures leave the old
checkout in place; failed publication attempts to restore it. A failed rollback
reports the retained backup path. Offline restoration can repair a locally
available commit but never acquires a missing checkout or commit.

These operations do not edit settings. They reject local sources, npm sources,
unpinned Git sources, symlinked managed paths and redirected Git metadata. They
accept sources and scopes, never arbitrary cleanup paths. Restoration deliberately
discards edits inside managed remote checkouts; use a local source for development.
Ordinary `install` keeps its existing behavior and does not imply an integrity
check.

## Release inspection

`list_releases(source)` lists a Git source's tags in name order, pairing each name
with its peeled commit ID. Annotated and lightweight tags each produce one row.
An untagged repository returns an empty list. The caller chooses the release;
this operation does not apply version policy or change a checkout.

`read_candidate_files(source, paths)` requires an explicit Git pin. It clones and
selects that candidate in an exclusively owned directory below the temporary
package scope, reads the requested relative files, then removes the staging
directory before returning. Results preserve input order and raw bytes; a missing
file is `None`. Absolute paths, parent traversal and links outside the candidate
are rejected. Read and cleanup failures are returned.

Candidate inspection does not install dependencies, run lifecycle scripts, build
code or load extensions. Each call has its own checkout, so alternating pins
cannot change an installed checkout or the ordinary temporary checkout. An
accepted candidate still installs through `install` with normal dependency
preparation.

Release inspection rejects non-Git sources and reports remote failures. Offline
mode returns `NotConnected` without running a command rather than treating an
unavailable release list as an empty repository. Settings stay unchanged.

## Progress

`set_progress_callback` replaces or clears the callback; each emission reads the
current one, so a replacement made by a callback or while a child runs applies to
the next event. An operation reports a start event (`Installing <source>...` or
`Removing <source>...`), then a completion event or an error event carrying the
failure text. A failing start callback stops the operation without an error event; a
failing completion callback is reported as an error; a failing error callback
replaces the error.

## Commands

`maestro-cli`'s `handle_package_command` turns `install`, `remove` (with
`uninstall` as its alias), `update` and `list` into those operations and prints
their outcomes; it returns whether the first argument named a package command.
Each command loads settings lazily, reports their failures as
`Warning (package command, <scope> settings): <message>`, keeps the raw global
snapshot for self-update, and only then builds the manager. A warning callback may
edit the live settings before that snapshot; later factory, progress and output
edits do not replace it.

Options are scanned left to right and keep the first value of each diagnostic
category: unknown option, missing value, extra argument, conflict, then a missing
install or remove source. `--help` wins over all of them and prints the complete
document of its command. `--local` applies to install and remove only; `--self`,
`--extensions`, `--extension <source>` and `--force` apply to update only. An empty
first positional source is replaceable, while an empty extra argument is present
and reports `Unexpected argument .` before any settings read or package effect.

Update runs the selected extension update first and the application update second,
so an extension failure stops before any self-update. The self-update side reuses
the [application plan and runner](application/configuration.md#self-update-selection):
a release that is not newer skips it, a changed package identity replaces the
installed one, and a nonzero child exit reports the manual command to run.

`handle_config_command` handles one leading `config` and ignores later tokens. It
observes the working directory and the agent directory once, loads the settings
owner, reports their failures as
`Warning (config command, <scope> settings): <message>`, builds the manager over
that same owner, resolves the configured paths and presents the
[selector](cli.md#selectors) over them. A command whose presentation succeeds asks
that presentation's exit effect for status 0; directory, settings, manager,
resolution and presentation failures keep their own error, so it requests no exit and
prints no package-command error line.

## Persistence

`install_and_persist` records the normalized source after successful acquisition.
If the first matching package has a different stored spelling, only its source is
replaced; its other fields and list position stay intact. An identical spelling
changes nothing. An absent source is appended. The same rules apply when a pin or
Git checkout is unchanged. See [Git preparation and recovery](package-sources.md#git-preparation-and-recovery).
`remove_and_persist` removes the source after a successful removal and returns
whether any entry changed. Neither operation flushes settings nor undoes installed
contents when the settings edit fails.

Before progress, child processes or file changes, explicit install and remove
operations ask the registered decider and check the persisted base layer for
their scope. Global operations use the global layer only; project operations use
the project layer when present, otherwise the global layer. A packages lock
refuses installation. Removal refuses an identity listed in that layer's package
entries, even when the requested pin differs. Refusals name the release and its
recorded time.

When a decider or base layer is present, persisting operations also prepare their
exact preference edit before acquisition. After contents and progress effects
succeed, the package list is recomputed from current settings and applied within
the admitted key and scope, without a second decision. Without either, preference
validation and reads occur after acquisition.

## Children

Every package child starts through the native launcher with an argument vector,
without Maestro building a shell command string. On Windows the launcher finds a
command such as `npm` using the child's working directory, `PATH` and `PATHEXT`.
Relative command paths and search directories use that working directory;
capture overrides apply to lookup as well as the child's environment. Resolution
and spawning share one operation. Rust's standard library runs `.cmd`
and `.bat` files through `cmd.exe` and handles batch argument escaping.

`NativePackageOperations::run_command` runs a child with inherited streams, or, while
the supplied query reports stdout taken over, with stdin ignored and output sent to
standard error. It completes when the child exits, whether or not its streams are
still open, and must run inside a Tokio runtime with process support. It returns the exit code,
`None` when a signal ended the child, and keeps a spawn failure's native error. The
package manager turns a nonzero or missing code into the failure
`<command> <arguments> failed with code <N or null>`.

## File patterns

The pure [file-pattern operation](package-patterns.md) is available
separately from package discovery.

## Conventional resource discovery

`PackageManager::resolve_extension_sources` processes explicit local, npm and Git
sources sequentially; `None` selects user scope. Project scope uses the project
configuration directory, and temporary local sources use the manager's working directory.
See [configured sources](package-sources.md) for source parsing and local path rules.

Missing remote sources are acquired without install progress or settings edits.
A pinned npm source is also acquired when its installed nonempty version string
differs literally from the authored suffix. Pin comparison does not resolve tags or
ranges; acquisition forwards the authored spec to the configured package command.
Offline acquisition skips that source, including mismatched installed pins.
Existing unpinned npm sources are collected without a version probe.
Acquisition errors stop resolution before later sources.

Temporary npm shares `<temp>/maestro-extensions/npm/f35b2129`.
Temporary Git uses `<temp>/maestro-extensions/git-<host>/<hash>/<path>`;
`hash` is the first eight lowercase hexadecimal characters of SHA-256 over
`git-<host>-<path>` before lexical joining, without the ref.
Rust's OS temporary-directory selection (`TMPDIR` on Unix) gives the temporary base.
Temporary Git creates no managed-root ignore file. Cache files outlive the manager.

Only existing, unpinned, online temporary Git sources refresh. The refresh rechecks
offline mode, emits pull progress with `Refreshing <source>...`, and delegates to
[Git update execution](package-updates.md#configured-execution).
Refresh failures, including progress-callback failures, are suppressed;
collection uses surviving contents outside that catch. Resource collection errors
from remote sources propagate. Ordinary install commands have no added deadline;
[captured commands](package-updates.md) retain their existing deadline.

Conventional directories are `extensions`, `skills`, `prompts`, `themes` and
`contributions`. Without a present package-root manifest, a local directory with
none of these paths is returned as an extension root; an existing conventional
path suppresses that fallback even when discovery is empty.
Extension entry-point manifests use `package.json`'s `maestro.extensions` literal
paths before `index.ts`, `index.js` or `index.wasm`, in that order. Other extension
discovery admits top-level `.ts`, `.js` and `.wasm` files and entry-bearing children.
Suffix matching is case-sensitive; a child's helper files are not implicit entries.
These entry rules belong to `maestro_resources::package_manifest`, which
[extension path discovery](extensions/loading.md) shares. Skills stop at an admitted `SKILL.md`;
loose Markdown is root-only. Prompts recurse for `.md`; themes and contributions
recurse for `.json`.

Discovery reads `.gitignore`, `.ignore` and `.fdignore` in that order, using
directory-scoped rules from `maestro-pattern` and ignored-parent pruning. Rules match Unicode scalars
with one-to-one uppercase equivalence; expansions and non-ASCII-to-ASCII mappings
remain distinct. A question wildcard consumes one scalar, and bracket complements
apply to the complete case-equivalent member set. Braces and parentheses are
literal in ignore rules. Discovery discards blank lines and lines whose trimmed
text begins with `#` before rule preparation removes one leading byte-order mark.
Directory patterns such as `a/**/` match `a` itself; `a/**` selects only descendants.
The deepest matching directory decides, with later rules winning
within that directory. Native Unix
listings use byte-name order. Returned records keep their authored path spellings; configured ownership is
described in [configured resource resolution](#configured-resource-resolution).
Delegated path rules belong to [configured sources](package-sources.md) and
file-pattern policy to [file patterns](package-patterns.md).

## Package-root manifests

An object `maestro` member in `package.json` selects resources instead of
conventional discovery. An absent or non-object member uses conventions, including
numbers, booleans, strings and arrays. Object members select `extensions`,
`skills`, `prompts`, `themes` and `contributions`. Omitted lists add
no resources. A list that is present and truthy but is not a list of strings is
ignored whole and adds no resources; the other lists still load, and nothing
fails. One reader, `maestro_resources::package_manifest::Manifest::entries`, reports
such a list as `InvalidData`: package resolution, filtered packages and conventional
package discovery ignore that report, while extension loading fails on a non-string
item.

Positive manifest entries containing `*` or `?` expand against the package root
without trimming. Other entries, including brace-only or bracket-only forms,
remain literal paths. Override prefixes are classified before expansion.
Within each selected resource list, positive entries expand before that list
is collected; expansion does not read resource ignore files. Direct files need no suffix check; matched
directories use the [conventional collectors](#conventional-resource-discovery).
Within each directory phase of an expansion, literal matches precede scans and
current matches precede recursive subwalks; duplicate authored paths retain their first position.
Expansion resolves its root and seeds its ancestry before walking. Literal child
lookups participate in the first successful directory listing; subsequent scans
reuse that listing’s names and order, without caching entry metadata. Pending
literal names compare lowercased text on Windows and macOS, and exact text
elsewhere; promotion retains the authored path spelling. Authored identities are
not Unicode-normalized. The separately
selected [filesystem policy](package-patterns.md#filesystem-expansion-policy)
owns pattern preparation and case selection.

Exclusions (`!pattern`) run before exact restorations (`+path`), with exact
removals (`-path`) last. Exclusions use the existing
[file matcher](package-patterns.md) on relative, basename and absolute views;
`SKILL.md` also offers its parent directory. Exact overrides compare relative or
absolute paths, including skill-parent paths, after stripping one `./` or `.\`
prefix. Selection inserts admitted candidates in their original candidate order.
Without exclusions or removals, candidates remain admitted without matching,
including when only restoration modifiers are supplied.

Lists are consumed in extension, skill, prompt, theme and contribution order.
A malformed list contributes nothing in an unfiltered package, and the remaining
kinds and sources still contribute. In an object-filtered package, a kind without
a filter uses its manifest list only when it is a list of strings. A malformed
list counts as absent, so that kind uses its conventional directory. An empty
list is present and contributes nothing. These rules apply to local and installed
remote packages alike.

## Configured resource resolution

`PackageManager::resolve` captures user and project settings before acquisition.
It selects project packages before user packages, keeping the first registration
of each source identity. Source identity belongs to [configured
sources](package-sources.md); acquisition reuses the operations described above.

The optional borrowed missing-source callback runs sequentially for missing remote
contents or a mismatched npm pin, after the offline check. `MissingSourceAction::Install`
acquires before collection, `Skip` omits the source, and `Error` returns
`Missing source: <source>` with the stored spelling. Without a callback, online
resolution acquires; offline resolution omits sources needing acquisition without
calling the callback. Callback and acquisition failures stop resolution. No install
progress or settings edit is added by resolution acquisition.

Package object filters select each resource kind independently. An omitted filter
falls back to its conventional directory when the manifest list is absent, null,
false, zero or an empty string; otherwise it uses that list. A present filter selects from manifest-admitted candidates, falling back
to conventions when that manifest list is empty or absent; an empty filter disables
those candidates. User exclusions retain disabled records, whereas manifest
exclusions remove candidates before user filtering. A local file remains an enabled
extension regardless of package filters. A selected malformed filter keeps prior
local kinds and ends that source's discovery; remote filter failures propagate.

Top-level resource entries use the user configuration directory or the project's
`.maestro` directory as their base. Only a leading `!`, `+`, `-`, or any `*` or `?`
classifies a pattern; patterns select discovered literal-entry candidates, not new
files. Positive patterns select matches before exclusions, exact restorations and
exact removals, using the [file-pattern operation](package-patterns.md). Disabled
candidates remain in the returned lists.

Automatic roots are the five conventional directories in each configuration base.
Automatic prompts, themes and contributions are shallow; configured document
directories recurse.
Conventional skill roots admit loose root Markdown, while `.agents/skills` roots
admit skill-directory entries. The project also visits `.agents/skills` from its
working directory through the nearest `.git` marker, inclusive, or to the filesystem
root; the home `.agents/skills` root is user-scoped. Its discovery paths keep the
joined home-root spelling; only the ancestor comparison resolves that home root.
Exact overrides can therefore select the authored home paths. Automatic selection
uses only modifier overrides from that scope's resource list, not positive patterns.

For each resource kind, identical authored paths retain the better precedence
owner, keeping the first on equal rank and replacing at the existing key position.
Stable ranking orders project local, project automatic, user local, user automatic,
then package records; canonical deduplication retains the first ranked record,
including when it is disabled. Filtered manifest candidates use survivor-then-restoration
order; unfiltered manifest and top-level collectors emit their original candidate order
before authored-path merging and ranked canonical deduplication.

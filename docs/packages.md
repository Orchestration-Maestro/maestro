# Package installation and removal

`PackageManager::install` and `remove` acquire or delete the contents of one source
for the user scope or, with `Some(InstalledSourceScope::Project)`, the project scope;
`None` means user. The `*_and_persist` variants then edit settings. Source identity,
local path rules and settings storage belong to [configured package
sources](package-sources.md), `maestro-path` and `maestro-settings`.

## Sources

- **npm** (`npm:` prefix): the user scope runs `install -g <spec>` or
  `uninstall -g <name>`. The project scope works in `<project>/.maestro/npm`:
  install first creates the root, a `.gitignore` of `*` and `!.gitignore`, and a
  `package.json` naming `maestro-extensions`, each only when missing, then runs
  `install <spec> --prefix <root>`; removal runs `uninstall <name> --prefix <root>`
  only when the root exists. The command is `npm` unless settings configure a
  nonempty `npmCommand` array, whose leading arguments come first (an empty array
  selects `npm`). An empty first entry fails with
  `Invalid npmCommand: first array entry must be a non-empty command`.
- **Git**: a target that already exists, directory or file, ends installation at
  once. Otherwise the repository is cloned below the scope's `git` root (with a `.gitignore` there),
  checked out when a ref was given, and its dependencies installed when it has a
  `package.json` (`install --omit=dev`, or plain `install` when a nonempty
  `npmCommand` is configured). The settings are read after the clone. Removal deletes the target and
  then empty ancestors strictly inside the Git root, comparing path components after
  resolving both against one working-directory read taken only for a relative base; a failed ancestor removal ends the cleanup.
- **Local paths**: installation only checks that the path, resolved against the input
  base, exists (`Path does not exist: <path>` otherwise); removal does nothing.

Installation does not consult the offline setting; that belongs to resource
resolution.

## Progress

`set_progress_callback` replaces or clears the callback; each emission reads the
current one, so a replacement made by a callback or while a child runs applies to
the next event. An operation reports a start event (`Installing <source>...` or
`Removing <source>...`), then a completion event or an error event carrying the
failure text. A failing start callback stops the operation without an error event; a
failing completion callback is reported as an error; a failing error callback
replaces the error.

## Persistence

`install_and_persist` adds the source to settings after a successful install and
ignores an existing entry. `remove_and_persist` removes it after a successful removal
and returns whether any entry changed. Neither flushes settings nor undoes
installed contents when the settings edit fails.

## Children

`NativePackageOperations::run_command` runs a child with inherited streams, or, while
the supplied query reports stdout taken over, with stdin ignored and output sent to
standard error. It completes when the child exits, whether or not its streams are
still open, and must run inside a Tokio runtime with process support. It returns the exit code,
`None` when a signal ended the child, and keeps a spawn failure's native error. The
package manager turns a nonzero or missing code into the failure
`<command> <arguments> failed with code <N or null>`.

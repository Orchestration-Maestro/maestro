# Application configuration

`maestro_app::configuration` owns compiled installation identity, path queries,
installation-method selection, the native user agent, startup timing samples and
[hosted service operations](services.md).
The SDK root exports `VERSION` and native `get_agent_dir`.

## Identity and roots

`config` supplies the package/command name `maestro`, display title `Maestro`,
configuration directory `.maestro` and the Cargo package version. It derives
`MAESTRO_CODING_AGENT_DIR` and `MAESTRO_CODING_AGENT_SESSION_DIR` from the command
name. `SERVICE_ORIGIN` is empty; passing it to the hosted service operations
produces no request or share link. Callers may supply one nonempty origin to
enable those operations.

`config::paths::get_agent_dir` uses a nonempty supplied override, otherwise
`<home>/.maestro/agent`. Only exact `~` or a leading `~/` asks for home when an
override is selected; expansion preserves the suffix text. The native wrapper
reads the agent override and reuses the native home lookup only when needed.
User-path getters join their suffix onto the supplied root, without rediscovery.
[Settings](../settings.md), [credentials](../credentials.md) and
[sessions](../sessions/session_format.md) retain their supplied-path interfaces.
See [default resource routes](defaults.md) for caller-level selection boundaries.

## Installation paths

`paths::get_package_dir` takes an `Installation` with module directory, executable
path and `PackageLayout`. A nonempty package override wins. Otherwise standalone
layout uses the executable directory; package layout walks ancestors from the module
directory, checking `package.json` before `Cargo.toml` at each non-root directory.
No marker returns the original module directory; the filesystem root is not probed.

The native getters take no arguments and describe the running process.
`Installation::current` uses the running executable and this crate's compile-time
directory. Its layout is package when the walk from that directory finds a marker,
as it does for a build run from its source, and standalone otherwise.
`config::get_package_dir` applies the `MAESTRO_PACKAGE_DIR` override to it.
`config::get_readme_path`, `config::get_docs_path` and `config::get_examples_path`
resolve their paths in that package directory. `config::get_theme_directories`
pairs its shipped theme directory with the custom theme directory of the native
agent directory; see [application resources](resources.md#theme-directories).

Standalone assets use adjacent `theme`, `export-html` and `assets` directories.
Package assets use `src` when that child exists, otherwise `dist`. Missing assets
still produce paths. README, documentation, examples and changelog queries use
[`maestro-path`](../../crates/maestro-path/src/lib.rs) lexical resolution with
the process directories; they read the working directory only when they need it,
and propagate its error. Other path joins also use that shared path owner.

`detect_install_method` inspects both supplied paths, normalizing separators and
ASCII case for manager markers. Selection is standalone, pnpm, Yarn, Bun global,
npm, then unknown. Detection never executes a package manager.

## Runtime observations

`maestro_user_agent::get_maestro_user_agent` formats
`maestro/<version> (<platform>; rust; <arch>)`, translating native target aliases
while retaining the supplied version text.

`timings::Timings` is caller-owned. Only the exact environment value `1` enables
profiling. Construction reads the clock once even when disabled; later disabled
operations read neither clock nor writer. Enabled samples retain label text,
order, duplicates and signed deltas. Printing keeps samples; reset clears them
and reads a new clock only when enabled. Printing returns the original writer
error. Callers supply the clock and stderr writer; no global timing state is added.

## Self-update selection

On native targets, `config::get_self_update_command` takes a borrowed
`SelfUpdateContext`, the installed package name, optional configured npm argv,
and an optional replacement package name. It returns an owned `SelfUpdateCommand`
only for an evidenced global installation whose package directory and immediate
parent pass write-access checks. A replacement contains removal then installation
in `steps`; otherwise callers execute the top-level command and literal `args`.
The display is human-readable text, not a POSIX shell command to execute.

The selector reuses installation detection and caller-selected package roots.
It supports npm, pnpm, Yarn and Bun; unconfigured npm can infer a matching
`lib/node_modules` prefix. Configured npm argv disables that fallback. Ownership
uses existing canonical paths with a separator-delimited strict descendant
comparison, case-folded only on Windows. Supplied `SelfUpdateOperations` control
filesystem, host flavor, home, access and read-only query effects. Required
configured-query errors retain the native cause and command context. Authored
paths use the shared flavor-explicit resolver, so supplied POSIX and Windows
observations work independently of the compilation host. Directory observations
remain lazy: anchored paths need neither drives nor the working directory.

`get_update_instruction` returns a manual command without ownership/access
probes. `get_self_update_unavailable_instruction` explains unmanaged or unwritable
installations and gives standalone release-download guidance. These functions
select descriptions only: no installation, removal or release lookup is run.

`NativeSelfUpdateOperations` delegates query capture to the process owner without
a deadline or buffer limit. Queries pass the executable and literal argument
vector to the native process API, not through joined shell text. On Windows,
executable lookup follows the [native launch rules](../../crates/maestro-process/README.md#command-preparation).
Unix access uses the OS write-access operation;
Windows checks existence and read-only attributes, not directory ACLs, so a later
update may still fail with a permissions error. See the
[configuration library notes](../libraries/application-configuration.md).

## Workspace preferences

Workspace preference loading uses the application's existing `SettingsManager`
and supplied agent directory through `maestro_daemon::settings::io`. Only accepted
global `multiplexer`, `daemon` and `update` members are projected; project chat
settings retain their existing merge. The loader does not discover another root,
create another manager or read another configuration file. See the
[workspace defaults and reload contract](../settings.md#global-workspace-projection-and-reload).

## Daemon paths

`maestro_daemon::paths::DaemonPaths` retains the supplied agent directory and a
validated optional daemon name. Construction creates no files and does not
normalize the root. The reserved `default` name uses the agent root; other names
use `<agent dir>/daemons/<name>/`.

| Resource | Relative path | Root |
|---|---|---|
| API and client sockets | `daemon.sock`, `daemon-client.sock` | Selected daemon |
| Topology and history snapshots | `daemon-snapshot.json`, `daemon-snapshot-history.json` | Selected daemon |
| Role logs | `maestro-in-process.log`, `maestro-client.log`, `maestro-daemon.log` | Selected daemon |
| Detection overrides | `detection/<agent>.toml` | Agent directory |
| Detection catalog | `detection/catalog/` | Agent directory |
| Catalog status and remote manifests | `detection/catalog/status.toml`, `detection/catalog/remote/<agent>.toml` | Agent directory |
| Action configuration and state | `workspace-actions/<encoded id>/config`, `workspace-actions/<encoded id>/state` | Agent directory |
| Announcement state | `announcements.json` | Agent directory |
| Remote resources | `remote/` | Agent directory |

Detection paths use canonical agent labels. `paths::action_path_component` encodes
arbitrary action identities into one component, including separators, percent
signs, Windows reserved stems and trailing dots. Long components keep a prefix
and a SHA-256 suffix within 120 ASCII bytes. Action configuration and state share
this encoder. Socket overrides are supplied to the existing native IPC selector;
settings and package roots stay with their existing owners.

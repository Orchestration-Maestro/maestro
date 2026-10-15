# Application configuration

`maestro_app::configuration` owns compiled installation identity, path queries,
installation-method selection, the native user agent and startup timing samples.
The SDK root exports `VERSION` and native `get_agent_dir`.

## Identity and roots

`config` supplies the package/command name `maestro`, display title `Maestro`,
configuration directory `.maestro` and the Cargo package version. It derives
`MAESTRO_CODING_AGENT_DIR` and `MAESTRO_CODING_AGENT_SESSION_DIR` from the command
name. `SERVICE_ORIGIN` is empty; this module performs no service requests.

`config::paths::get_agent_dir` uses a nonempty supplied override, otherwise
`<home>/.maestro/agent`. Only exact `~` or a leading `~/` asks for home when an
override is selected; expansion preserves the suffix text. The native wrapper
reads the agent override and reuses the native home lookup only when needed.
User-path getters join their suffix onto the supplied root, without rediscovery.
[Settings](../settings.md), [credentials](../credentials.md) and
[sessions](../sessions/session_format.md) retain their supplied-path interfaces.
See [default resource routes](defaults.md) for caller-level selection boundaries.

## Installation paths

The caller supplies `Installation` with module directory, executable path and
`PackageLayout`. A nonempty package override wins. Otherwise standalone layout
uses the executable directory; package layout walks ancestors from the module
directory, checking `package.json` before `Cargo.toml` at each non-root directory.
No marker returns the original module directory; the filesystem root is not probed.

Standalone assets use adjacent `theme`, `export-html` and `assets` directories.
Package assets use `src` when that child exists, otherwise `dist`. Missing assets
still produce paths. README, documentation, examples and changelog queries use
[`maestro-path`](../../crates/maestro-path/src/lib.rs) lexical resolution with
supplied `ResourceOperations`; a working directory is requested only when needed,
and its error is propagated. Other path joins also use that shared path owner.

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

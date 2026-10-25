# Package updates

Call `DefaultPackageManager::check_for_available_updates` on an `Rc` manager to
check installed, unpinned npm and Git sources. Returned `PackageUpdate` records
retain the configured source spelling, display name, kind and scope. Project
registrations precede user registrations; the first source for each identity
wins. Source identity and path handling belong to [configured package
sources](package-sources.md).

Local sources, explicit npm versions/tags/ranges, Git refs and missing installed
contents produce no update record. npm compares the installed manifest's nonempty
string version against the decoded latest string exactly. Missing, unreadable or
invalid installed versions produce no record. Failed latest-version and Git
probes are skipped; consumed source, scope-root and installed-path lookup failures
can reject the operation. Checks do not install, persist settings or emit progress.

`MAESTRO_OFFLINE` enables offline mode only for `1`, case-insensitive `true` or
case-insensitive `yes`, without trimming. Offline returns before reading settings;
individual npm/Git checks read the value again before probing.

## Configured execution

Call [`PackageManager::update`](../crates/maestro-packages/src/package_manager/mod.rs)
on an `Rc` manager. Absent or empty input selects all registrations; named input
uses [configured source identity](package-sources.md). Selection retains registration order and duplicates without writing settings. Selection precedes the
offline return, so an unmatched named input still fails offline and can suggest
the first configured shorthand match.

Matched local sources and explicit npm versions/tags/ranges or Git refs are skipped
before inspecting installed contents. Unknown installed npm versions and failed
latest-version lookups cause an update, unlike availability checking. npm batches
preserve candidate order as `name@latest` within each scope. After npm checking,
nonempty scope batches and Git work are admitted concurrently through the same
private worker helper used for availability. Command configuration and progress
callbacks are read live during execution.

Missing unpinned Git contents use the existing installation operation. Existing
contents select an origin upstream or origin-HEAD fallback, fetch its targeted
ref, and compare refreshed and local heads. Equal heads preserve dirty and
untracked contents; different heads cause reset, clean and optional dependency
installation. Failed preparation restores the previous revision when recovery
succeeds, so the next update retries. See [Git preparation and recovery](package-sources.md#git-preparation-and-recovery)
for failure reporting and in-place limitations.

## Native runtime

Pass a caller-owned `Rc<tokio::task::LocalSet>` to `NativePackageOperations::new`
and drive it inside a Tokio runtime with process and timer support. The adapter
retains the local set weakly. `NativePackageOperations::with_spawn` instead accepts
owned-work admission from a caller's existing runtime, returning its admission
errors unchanged. Both constructors use the same native command implementations.
The interactive mode supplies weak terminal-runtime admission and starts the
immediate prefix before its input hooks run.

Up to four workers preserve input result order, even when
checks finish out of order. The first worker error rejects the check; admitted
siblings continue claiming sources while the caller drives the local runtime.
A dropped runtime rejects admission when work is required; an empty or offline
check needs no admission.

## Captured commands

`PackageOperations::run_command_capture` takes literal arguments and
`CommandCaptureOptions` for cwd, an optional deadline and environment overrides.
Native executable lookup uses that child directory and environment before spawning.
Native capture returns trimmed stdout after child exit and both pipe EOFs;
success does not substitute stderr. It ignores stdin and does not query stdout
takeover. The captured child, its pipes and termination come from
[`maestro-process`](../crates/maestro-process/README.md); this page's deadline,
decoding and error rules stay with packages. Invalid UTF-8 uses replacement decoding after collecting bytes.

Availability captures use a 10000 ms deadline. Capture deadlines remain active
through pipe EOF, including pipes retained by descendants. Expiration requests
ordinary termination once, with no escalation or descendant-kill policy.
Capture errors use `<command> <arguments> timed out after <milliseconds>ms` or
`<command> <arguments> failed with <code or signal>: <stderr-or-stdout>`; nonempty
stderr takes precedence without trimming. Spawn failures retain native errors.
The internal empty npm response and unmatched remote HEAD errors are suppressed
by their availability probes.

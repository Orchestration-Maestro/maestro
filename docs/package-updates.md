# Available package updates

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

## Native runtime

Pass a caller-owned `Rc<tokio::task::LocalSet>` to `NativePackageOperations::new`
and drive it inside a Tokio runtime with process and timer support. The adapter
retains the local set weakly. Four workers preserve input result order, even when
checks finish out of order. The first worker error rejects the check; admitted
siblings continue claiming sources while the caller drives the local runtime.
A dropped runtime rejects admission when work is required; an empty or offline
check needs no admission.

## Captured commands

`PackageOperations::run_command_capture` takes literal arguments and
`CommandCaptureOptions` for cwd, an optional deadline and environment overrides.
Native capture returns trimmed stdout after child exit and both pipe EOFs;
success does not substitute stderr. It ignores stdin and does not query stdout
takeover. Invalid UTF-8 uses replacement decoding after collecting bytes.

Availability captures use a 10000 ms deadline. Capture deadlines remain active
through pipe EOF, including pipes retained by descendants. Expiration requests
ordinary termination once, with no escalation or descendant-kill policy.
Capture errors use `<command> <arguments> timed out after <milliseconds>ms` or
`<command> <arguments> failed with <code or signal>: <stderr-or-stdout>`; nonempty
stderr takes precedence without trimming. Spawn failures retain native errors.
The internal empty npm response and unmatched remote HEAD errors are suppressed
by their availability probes.

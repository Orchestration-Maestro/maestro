# Application package access

The native application SDK forwards `DefaultPackageManager`, `PackageManager`,
`PathMetadata`, `ProgressCallback`, `ProgressEvent`, `ResolvedPaths` and
`ResolvedResource` from the [package owner](../packages.md).
Use the [selected configuration root](configuration.md) with the existing
file-backed settings constructor and package manager options. Registration,
filters, acquisition and [updates](../package-updates.md) stay with their owners.
Discovery accepts author-built components without building or activating them.

## Managed standard output

`maestro_app::output_guard` reserves standard output for protocol traffic with
`take_over_stdout` and releases it with `restore_stdout`. Both are idempotent;
`is_stdout_taken_over` reads their shared process-wide state.

Ordinary application output uses `write_stdout`, which selects standard error
while stdout is reserved. `write_raw_stdout` always writes to standard output.
Neither adds a newline or changes the supplied text. `flush_raw_stdout`
synchronously flushes standard output. Writes and flushing return native I/O
failures. This owner controls managed writes, not unrelated libraries or native
file descriptors.

Construct `NativePackageOperations` with `Rc::new(is_stdout_taken_over)` so
its existing child launcher reads the state at each invocation. Keep the
caller-driven local runtime alive through those operations; see
[package process routing](../packages.md#children).

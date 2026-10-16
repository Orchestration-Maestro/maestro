# Application configuration libraries

The native self-update selector uses Unix-only `rustix =1.1.5`, with default
features disabled and `std,fs` enabled, for `fs::access` with `WRITE_OK`.
It checks package and parent access without creating or truncating a probe file.
The dependency is already locked in the workspace and is used under MIT or
Apache-2.0; no new package version is introduced.

Process capture reuses `maestro-process` and the existing native Tokio dependency.
Filesystem observations reuse `maestro-resources`; lexical paths and whitespace
reuse `maestro-path` and `maestro-text`. Windows uses standard-library metadata
for its attribute/existence observation, not an ACL guarantee. See
[application configuration](../application/configuration.md#self-update-selection)
for the selector's caller contract.

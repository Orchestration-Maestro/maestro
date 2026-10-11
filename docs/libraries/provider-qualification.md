# Provider qualification libraries

Native message qualification in `maestro-models` uses the test-only dependency
`icu_collator =2.3.1` with default features, including `compiled_data`, under
Unicode-3.0. It supplies locale comparisons for source-owned provider probe
selection; it does not change catalog enumeration or runtime model routing.
The dependency version is already resolved elsewhere in the workspace.

The resolved collation closure contains 36 packages including the root, with
Unicode-3.0, MIT and Apache-2.0 license expressions. Existing request capture,
message framing, runtime driving and isolated test processes are reused;
no additional transport, parser, scheduler or normal dependency is added.

See [controlled conformance](../models/conformance.md#controlled-message-conformance)
for the exercised inputs and service-qualification limits.

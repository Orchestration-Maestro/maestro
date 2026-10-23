# Application test support

[Testing the application session](../application/testing.md) describes the
local session harness, the scripted provider, the waiting rules, the fixtures
and cleanup.

Broad lifecycle and behaviour checks for the session live in
`crates/maestro-app/tests/`, one module per area of behaviour, all registered
in `tests/integration.rs`. A regression check sits beside the cases it
protects and is named after the behaviour it keeps, never after an issue
number.

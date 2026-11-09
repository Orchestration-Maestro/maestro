# Diagnostics

## Quick start

Write a warning to standard error with `maestro_app::diagnostics::write_stderr_line`.
It adds the newline, writes once and returns nothing. If standard error is closed,
or its reader has gone, the text is dropped and your code carries on.

```rust
maestro_app::diagnostics::write_stderr_line("Warning: could not read the changelog");
```

Use `write_stderr` when the text already ends its own line.

## Why not `eprintln!`

`eprintln!` and `eprint!` panic when the write fails. A diagnostic is a courtesy,
so a dead terminal or a closed pipe must not turn one into a crash. Both writers in
`maestro_app::diagnostics` ignore the failure instead. The owner is
`maestro_agent::diagnostics`, the lowest crate the agent loop and the application
both reach; the application re-exports it for frontends.

Production code in the product crates uses these two functions for every line it
writes to standard error. Development tools and test harnesses, which only run in
a terminal you are watching, keep the print macros.

## Where Maestro writes diagnostics

| Producer | Text |
| -------- | ---- |
| Context and system prompt loading | `Warning: Could not read <path>: <error>`, in yellow on a terminal |
| Default extension bus | `Event handler error (<channel>): <error>` |
| Agent loop | the message of a rejected submission |
| Proxy stream | `Unhandled proxy event type: <type>` |
| Changelog parsing | `Warning: Could not parse changelog: <error>` |
| Server start | the running notice, or the already-running error |
| Startup failures and file arguments | the error, then exit status 1 |

## Testing a diagnostic path

`maestro_test_process::run_with_broken_stderr` runs a test in a child whose
standard error is a pipe with no reader. The child calls
`maestro_test_process::wait_for_broken_stderr` first, so no write can beat the
pipe's closing, then runs the code under test. The test passes when the child
exits with success.

```rust
if maestro_test_process::selection().is_child() {
    maestro_test_process::wait_for_broken_stderr();
    maestro_app::diagnostics::write_stderr_line("nobody reads this");
    return;
}
let status = maestro_test_process::run_with_broken_stderr(
    maestro_test_process::TestProcess::current("my_test").unwrap().case("*"),
)
.unwrap();
assert!(status.success());
```

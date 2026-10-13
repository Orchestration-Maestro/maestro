# Command execution and delays

`maestro-tools` runs an explicit program with `exec_command` and waits with `sleep`.
Both are native-only, like the [read tool](tools-read.md#read-tool); the crate still
builds for browser targets without them. Shell commands, the agent `bash` tool and
their rendering are not delivered here.

## Running a command

```rust,no_run
use maestro_tools::{ExecOptions, exec_command};

# async fn run() -> std::io::Result<()> {
let result = exec_command("git", &["status".into()], "/repo", ExecOptions::default()).await?;
println!("{} {}", result.code, result.stdout);
# Ok(())
# }
```

`command` and `args` are the literal program and argument vector: no shell
interprets them, stdin is closed, and the working directory is the positional `cwd`;
an empty `cwd` leaves the caller's working directory. An empty `command` fails with
`InvalidInput` before the signal is observed. The environment is the caller's.

`ExecOptions::signal` terminates the command when its
[`Cancellation`](../crates/maestro-models/src/lib.rs) is aborted, including one
aborted before the call. `ExecOptions::timeout` terminates it after that long; `None` and
zero set no deadline, and there is no default deadline. Termination is a request: the
command is killed if it is still running five seconds later, judged from its actual
state. The abort and the timeout together request termination once.

`ExecResult` holds `stdout` and `stderr` decoded separately from the bytes the wait
collected, with malformed UTF-8 replaced once over each collected stream, the exit `code` and `killed`
(whether the abort or the timeout requested termination). A command ended by a signal
has code 0.

A command that cannot start (missing program or directory, permission denied) is
not an error: it yields code 1 with empty streams. An operand the system rejects as
invalid (such as a NUL byte) returns the native `std::io::Error`. A failed wait yields
code 1 with the bytes already read.

A descendant that keeps the pipes open after the command exits does not hold the
result: at the latest 100 ms after the exit it is returned with what was read. The lifecycle is owned
by [`maestro-process`](../crates/maestro-process/README.md).

## Waiting

`sleep(duration, signal)` waits for `duration`. If `signal` is aborted first, including
before the call, it fails with `SleepError`, displayed as `Aborted`; a pre-aborted
call arms no timer. Finishing releases the signal observation.

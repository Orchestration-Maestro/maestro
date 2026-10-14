# Command execution and direct shells

`maestro-tools` runs an explicit program with `exec_command` and waits with `sleep`.
These operations and local shell operations are native-only, like the
[read tool](tools-read.md#read-tool); the crate still builds for browser targets
without them. The agent `bash` tool and its rendering are separate capabilities.

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


## Local shell operations

`create_local_bash_operations(bin_dir, shell_path)` returns replaceable
`BashOperations`. The application root re-exports `get_shell_config`,
`BashOperations` and `create_local_bash_operations` on native targets; their tools
paths remain available. Each `exec(command, cwd, options)` selects the supplied shell
path, or discovers bash (falling back to `sh` on Unix), then checks that `cwd`
exists before spawning with `-c` and the unchanged command. Selection checks
existence, not file type or executable permission; native spawn errors stay native.
The factory observes neither the filesystem nor inherited environment until a run.

`BashExecOptions::on_data` receives raw output chunks, without sanitization,
decoding or truncation. Stdin is closed. `env: Some(...)`, including an empty map,
replaces inheritance; omission reads fresh native entries and prepends the caller's
binary directory when absent from the first case-insensitive PATH entry. On
Windows, native insertion selects the first UTF-16-sorted spelling of each
Unicode-uppercase key identity; the supplied environment map stays unchanged.

`signal` is optional; `timeout` is optional positive seconds, with no default.
Zero, negative and unrepresentable durations arm no timer. Abort and timeout request
forced tree cleanup while the shared waiter continues draining. At successful
settlement an aborted signal wins over timeout; native spawn/wait errors win over
both. `BashError` preserves these categories and the original other error source.
`BashExecResult::exit_code` is absent for signal termination and otherwise numeric.

Unix local shells start a new session. The exported tracking helpers allow a
caller to request cleanup of tracked detached children; this library installs no
shutdown handlers. See [shared lifecycle behavior](../crates/maestro-process/README.md#streaming-and-sessions)
for streamed waits and exit grace.

```rust,no_run
use maestro_tools::{BashExecOptions, create_local_bash_operations};

# async fn run() -> Result<(), maestro_tools::BashError> {
let operations = create_local_bash_operations("/tool-bin".into(), None);
let mut output = Vec::new();
let mut sink = |chunk: &[u8]| output.extend_from_slice(chunk);
let result = operations.exec("printf hello", "/repo", BashExecOptions {
    on_data: &mut sink, signal: None, timeout: None, env: None,
}).await?;
assert_eq!(result.exit_code, Some(0.0));
# Ok(())
# }
```
## Helper launches

The application exposes native `spawn_sync`, `spawn`, `SpawnSyncOptions`,
`SpawnSyncError` and `OutputMode` from the
[process owner](../crates/maestro-process/README.md), plus the original
`exec_command`, `ExecOptions` and `ExecResult` above. Use capture options for
supplied input and independent stdout/stderr capture or discard, inherited options
for a caller-prepared terminal command, or the background launch for startup
acknowledgment without waiting for exit.

## Direct shell execution

`execute_bash_with_operations(command, cwd, operations, options)` invokes the supplied
`BashOperations` with the literal command and directory, the same optional signal,
and no timeout or replacement environment. Even a pre-aborted signal reaches the operation.

```rust,no_run
use maestro_tools::{BashExecutorOptions, create_local_bash_operations, execute_bash_with_operations};

# async fn run() -> Result<(), maestro_tools::BashError> {
let operations = create_local_bash_operations("/tool-bin".into(), None);
let result = execute_bash_with_operations("printf hello", "/repo", operations.as_ref(),
    BashExecutorOptions::default()).await?;
assert_eq!(result.output, "hello");
# Ok(())
# }
```

The optional synchronous `on_chunk` callback receives cleaned chunks in arrival order,
including empty raw chunks. Decoding uses the shared [streaming text primitive](text.md).
This caller drops the initial BOM, holds unfinished escapes between chunks, strips ANSI
sequences and binary controls, and removes carriage returns. Successful or cancelled
settlement flushes pending text; only nonempty final text adds a callback.

Raw output above 51200 bytes creates a sanitized `maestro-bash-{id}.log` artifact
in the native temporary directory. Retained output is a deque of whole cleaned chunks:
oldest chunks are removed above 102400 UTF-8 bytes while more than one remains.
Final selection uses the shared default tail limits; final truncation can also create
an artifact. `BashResult` returns the selected text, cancellation and truncation flags,
the optional supplied numeric status and the optional artifact path, without tool notices.
The signal is sampled after final output callbacks: an aborted signal selects cancellation
and no status; nonzero status alone is not an error.

An ordinary operation error returns unchanged without finalizing pending text or creating
a line-only artifact. The first processing failure stops output updates and returns after
the operation settles. Owned writers are flushed and closed; no durability is promised.

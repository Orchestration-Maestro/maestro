# Command execution and direct shells

`maestro-tools` runs an explicit program with `exec_command` and waits with `sleep`.
These operations and local shell operations are native-only, like the
[read tool](tools-read.md#read-tool); the crate still builds for browser targets
without them. The native agent `bash` tool is described [below](#agent-bash-tool); its rendering uses the same typed definition.

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
`BashOperations`, `BashExecOptions`, `BashExecResult`, `BashError` and
`create_local_bash_operations` on native targets; their tools paths remain
available. Each `exec(command, cwd, options)` selects the supplied shell
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
`ShellEnv` keeps optional native values: `Some(value)` sets a value, including an
empty string, while `None` leaves the key observable to hooks and supplied
operations but omits it from process creation. Key selection precedes omission,
so an unset Windows key does not expose another spelling of that same key.

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

For session settings and a dedicated Bash alias file, see
[Use aliases in shell commands](shell-aliases.md).

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
oldest chunks are removed above 102400 UTF-16 code units while more than one remains.
Final selection uses the shared default tail limits; final truncation can also create
an artifact. `BashResult` returns the selected text, cancellation and truncation flags,
the optional supplied numeric status and the optional artifact path, without tool notices.
The signal is sampled after final output callbacks: an aborted signal selects cancellation
and no status; nonzero status alone is not an error.

An ordinary operation error returns unchanged without finalizing pending text or creating
a line-only artifact. The first processing failure stops output updates and returns after
the operation settles. Owned writers are flushed and closed; no durability is promised.

## Agent bash tool

`create_bash_tool_definition::<C>(cwd, bin_dir, options)` builds a native typed
`ToolDefinition<C, BashToolInput, BashToolDetails, BashRenderState>`; `C` is unused.
`create_bash_tool(cwd, bin_dir, options)` uses the [definition wrapper](tools-read.md#tool-definitions)
to return an executable agent tool. Both factories and the input, details,
options and spawn-hook records are re-exported from `maestro-tools` and
`maestro-app` on native targets. The caller supplies the resolved binary directory;
[local shell operations](#local-shell-operations) own its environment insertion.

`BashToolOptions::operations` replaces the local operation, taking precedence over
`shell_path`. A nonempty `command_prefix` contributes its literal text and one
newline before the input command. Each call builds fresh environment operands,
then `spawn_hook` may replace the complete command, directory and environment;
a hook error returns before progress or execution. `BashToolInput::timeout` is
optional numeric seconds forwarded unchanged to the selected operation, as is
the optional cancellation signal; supplied operations own those policies once started.

An observer receives an empty-content update synchronously before the execution
future is returned, after a successful hook. While accumulation succeeds, the first
data callback publishes a snapshot immediately; later callbacks coalesce until 100 ms after the previous
emission, or flush on settlement. Data updates carry one text block and present
details, even when empty. Without an observer there is no progress timer.
After settlement the execution releases its observer and pending timer.
After the initial update, an already-aborted signal returns `Command aborted`
without entering either the local or supplied backend. An observer that aborts
that signal synchronously also prevents backend entry.

Output bytes are not sanitized. The [streaming accumulator](tools-read.md#native-streaming)
owns decoding, tail selection and raw artifacts. On successful accumulation the tool
finishes decoding and flushes dirty progress. It closes the artifact writer before
returning; closure can itself report an I/O error. If accumulation
fails, it stops consuming bytes, awaits the original operation's settlement and
reports the first I/O failure; it adds no stop policy or default timeout.

Zero or absent exit status succeeds. Empty successful output is `(no output)`;
nonzero status appends `Command exited with code ...`. Abort and timeout append
their respective status to output without inventing `(no output)`; timeout uses
the operation's seconds. Other errors return their diagnostic message without partial output appended;
native I/O codes are retained when available. Truncated final output includes a tail notice and details with
its full raw-output path. A raw-only spill can appear in progress details without
final truncation details.

### Bash rendering

Both definition callbacks read the published live theme. The call shows the authored
command with a nonzero numeric timeout suffix. Results sanitize ordered text and
[image fallbacks](tools-read.md#display-helpers), then trim boundary whitespace.
Collapsed results show the last five visual rows after [toolkit wrapping](terminal/widgets.md#text);
expanded results show the complete body. Hidden rows add a clipped hint using the
[current expand keybindings](terminal/keybindings.md). A retained hint reads current
bindings and colors when drawn, falling back to plain text if its theme becomes unavailable.

Nonempty artifact paths and active truncation add an independent warning after the
body. Execution-started calls capture the initial clock; results display `Elapsed`
while partial and `Took` otherwise, with one decimal second. Final or error results
freeze the first end time and cancel refresh. A partial error keeps the `Elapsed`
label. Historical results without a start show no clock. A later partial result
keeps a captured end time but resumes refresh until settled again.

`BashRenderState` owns one one-second refresh timer, cancelled when its last row
owner disappears. This timer uses the [shared toolkit runtime](terminal/widgets.md#loader)
and changes no execution, artifact or progress policy. Reused result components
refresh their body cache when content, expansion, width or explicit invalidation changes.

`truncate_to_visual_lines(text, maximum, width, padding_x)` is also available at the
tools and application roots on every target. It returns `VisualTruncateResult` with
selected padded rows and the number of earlier rows skipped. Absent padding means
zero; a zero maximum selects no rows. Wrapping and glyph clipping stay with [`Text`](terminal/widgets.md#text).

```rust,no_run
use maestro_tools::{BashToolInput, BashToolOptions, create_bash_tool, create_bash_tool_definition};

# async fn run() -> Result<(), maestro_models::DiagnosticErrorInfo> {
let definition = create_bash_tool_definition::<()>(
    "/repo".into(), "/tool-bin".into(), BashToolOptions::default(),
);
let result = (definition.execute)("call".into(), BashToolInput {
    command: "printf hello".into(), timeout: Some(1.5),
}, None, None, None).await?;
assert!(result.details.is_none());

let tool = create_bash_tool("/repo".into(), "/tool-bin".into(), BashToolOptions {
    command_prefix: Some("export MODE=fixture".into()),
    spawn_hook: Some(std::sync::Arc::new(|context| Ok(context))),
    ..BashToolOptions::default()
});
let result = (tool.execute)("call".into(), serde_json::json!({
    "command": "printf '%s' \"$MODE\"",
}), None, None).await?;
# Ok(())
# }
```

## Caller-local shell observers

The [caller-local factories](tools.md#caller-local-construction) select
`BashOperations<Local>` and local spawn hooks without changing command preparation
or progress ordering. Store operations with the family's shared pointer:
`E::Shared<<E as BashExecution>::Operations>` uses `Rc` for local operations and
`Arc` with `Send + Sync` bounds for threaded operations.
`BashExecOptions` borrows the selected raw callable; both native operation
implementations share one driver. The process leaf's
`wait_for_child_process_with_sink` accepts a mandatory borrowed sink without
`Send`, sharing capture, descriptor delivery and termination with the existing
`wait_for_child_process` interface. Its default `None` calls remain inferable.

## PTY children

On Linux, `maestro_process::pty::spawn` returns separate terminal-master and child
owners. `PtySetupEntry` supplies the executable and arguments that invoke the
process-owned child setup entry. Its `spawn` operation preserves native target
arguments, working directory and inherited or replacement environment policy.
The entry executable calls `setup_entry` before ordinary argument parsing; that
function returns `None` for ordinary startup and replaces a setup invocation with
its target. A returning setup error requires an unsuccessful process exit.
The lower-level `spawn` also accepts a caller-owned setup adapter.
`setup_and_exec` resets terminal signals, creates a session, acquires the controlling
terminal and replaces the setup process with the target. Setup failures retain their
original error text; the parent keeps one close-on-exec master after startup.

`PtyIoActor` runs on the current Tokio runtime. It delivers terminal bytes to an owned
callback, queues ordered replies and applies resizes before their attached replies.
User input has bounded capacity; geometry and handoff controls use an independent
route. Handoff drains accepted input before pausing reads and rejecting new input.
Rollback reopens admission; successful committed release stops I/O and joins the worker.
After an ordinary shutdown or natural exit, `wait_closed` joins that same worker.
The optional reader-exit hook returns an owned future. The actor awaits it before
releasing its master. Graceful shutdown keeps the runtime and notification consumers
alive until pending hooks finish; destroying the runtime cancels them and closes
their masters.

The platform-independent `foreground` module owns `ForegroundJob` and
`ForegroundProcess` observations. Linux retains their existing `pty::foreground`
paths as re-exports. The Linux-only `pty::foreground` module reads live process
groups, arguments, native working directories, raw environment values and session
membership. `session_processes_by_id` takes the original session identity, which terminal
setup establishes as the spawned child's PID; it does not need a live leader.
`session_processes` instead resolves a live member's session before enumerating it.
Detection mode and the environment-variable name come from the caller. See [pane processes](workspace-daemon.md#pane-processes).

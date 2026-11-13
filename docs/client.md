# RPC client

`maestro_app::RpcClient` runs Maestro in RPC mode as a child process and drives its
session through typed methods. The commands, answers and events it exchanges are
described in [the daemon protocol](daemon-protocol.md); this page covers what the
client itself does.

## Launching the child

`RpcClient::new(RpcClientOptions)` stores the options and launches nothing. `start`
launches the child, `stop` ends it, and a stopped client can start again.

- `cli_path` selects the executable. Without it the client runs `dist/maestro`
  (`dist/maestro.exe` on Windows). A relative path resolves against the child's
  working directory. The client never searches `PATH` and never builds anything.
- `cwd` sets the child's working directory; without it the child inherits the
  current one.
- `env` adds or replaces variables on top of the inherited environment.
- A nonempty `provider` or `model` adds `--provider` or `--model`. `args` come
  next, unchanged, after `--mode rpc`. No shell is involved.

`start` fails with `Client already started` while a child runs. A new child has
100 ms to fail. If it exits within them, `start` reports the exit as soon as the
child's diagnostic stream ends, with its exit code (`null` when a signal ended it)
and its diagnostic text. A descendant that keeps that stream open delays the report
to the end of the 100 ms, and the report then carries the text read so far. When the
100 ms are up, `start` checks once more for an exit before it reports the child as
started. A child whose start failed still counts as started until `stop`.

The client keeps the child's stderr as it arrives and a separate thread copies it
to the caller's stderr, so a slow reader of that stream never delays answers or
deadlines. `get_stderr` returns the text every child of the client has written so
far.

`stop` ends delivery of the child's output, sends the ordinary termination signal,
kills the child if it still runs one second later, and waits until the child is
reaped. Only the direct child receives a signal. `stop` also waits until the copy
has caught up with every stderr byte read so far. It does not wait for the stream to
end, because a descendant of the child can keep it open.

When `start` fails before the child runs, for example because the copying thread
cannot be created, the client stays unstarted and leaves no child behind.

The client runs on the caller's Tokio local executor, so use it inside a
`tokio::task::LocalSet`. The tasks that read the child's streams and drive the
deadlines live there.

## Commands

Each of the 29 session commands is a method, such as `prompt`, `get_state`,
`set_model` or `clone`. `clone` is the session command that duplicates the active
branch; the client itself is not cloneable. Requests are numbered `req_1`, `req_2`
and onwards, and the counter continues across restarts. Answers are matched to
their request by identifier, in any order.

A method fails with:

- the session's own message when the session answers with a failure, including for
  the commands that return nothing;
- `Client not started` without a child;
- `Timeout waiting for response to <command>. Stderr: <text>` when no answer arrives
  within 30 seconds;
- `Client stopped` when the client is stopped, or the child exits, before the answer
  arrives. A request sent while the client is stopping fails this way when the stop
  ends.

The first of these decides the outcome: the answer, the stop or exit, or the
30 seconds, counted from the moment the request is sent. A request awaited late
still gets that outcome, and a timeout carries the diagnostic text as it stood at
the deadline. An answer that arrives after the 30 seconds goes to the event callbacks.

`get_last_assistant_text` returns a `Presence<String>`: missing when the session has
no assistant message, and `null` or the text when it answers with one of them.

## Events

`on_event` registers a callback for every record the child writes that answers no
pending request, including records that are not objects. The client ignores lines
that are not JSON, and `null`. Callbacks run in registration order. Registering the same
callback twice delivers each record twice. A callback added during a delivery
receives that record too. A callback that fails stops delivery of that record to the
callbacks after it.

`on_event` returns the operation that removes the registration. Calling it again
changes nothing, and dropping it keeps the registration.

## Waiting for a run

`wait_for_idle` and `collect_events` register when called, so an `agent_end` that
arrived earlier does not count. `wait_for_idle` completes at the next
`agent_end`, and `collect_events` returns every event through it. Both wait 60
seconds unless given a timeout, counted from the call. They fail with
`Timed out while waiting for the agent to become idle. Stderr: <text>` or
`Timeout collecting events. Stderr: <text>`, and with `Client stopped` when the
child is stopped or exits first. A wait registered while the client is stopping
fails this way when the stop ends. Nothing that arrives after the deadline counts,
and a stop or an `agent_end` before it still counts when the wait is awaited later.

`prompt_and_wait` starts collecting before it sends the prompt, so events the
session writes before its answer are kept. When the prompt fails, the collection
ends with it.

Dropping a request or a wait releases only what it registered and sends nothing to
the session.

```rust,ignore
let client = RpcClient::new(RpcClientOptions {
    cli_path: Some("/opt/maestro/bin/maestro".into()),
    args: vec!["--no-session".into()],
    ..RpcClientOptions::default()
});
client.start().await?;
let events = client.prompt_and_wait("Summarize the README", None, None).await?;
client.stop().await?;
```

## Example

`rpc_client_demo` in maestro-packages' `examples/sdk` is an interactive terminal client
that uses these methods. From a maestro-packages checkout:

```sh
cargo run -p maestro-sdk-examples --example rpc_client_demo -- path/to/maestro
```

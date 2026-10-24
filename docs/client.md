# RPC client

`maestro_app::RpcClient` runs Maestro in RPC mode as a child process and drives its
session through typed methods. The commands, answers and events it exchanges are
described in [the daemon protocol](daemon-protocol.md); this page covers the client's
own behaviour.

## Launching the child

`RpcClient::new(RpcClientOptions)` only stores the options. `start` launches the
child and `stop` ends it; a stopped client can be started again.

- `cli_path` selects the executable. Without it the client runs `dist/maestro`
  (`dist/maestro.exe` on Windows). A relative path is resolved against the child's
  working directory. The client never searches `PATH` and never builds anything.
- `cwd` is the child's working directory; without it the child inherits the
  current one.
- `env` adds or replaces variables on top of the inherited environment.
- A nonempty `provider` or `model` adds `--provider` or `--model`. `args` follow
  them unchanged, after `--mode rpc`. No shell is involved.

`start` fails with `Client already started` while a child runs. It waits 100 ms and
reports a child that exited in that time, with its exit code (`null` when a signal
ended it) and its diagnostic text. That child still counts as started until `stop`.

The child's stderr is kept as it arrives and copied to the caller's stderr by a
separate thread, so a slow reader of that stream never holds up answers or
deadlines. `get_stderr` returns the text every child of the client has written so
far.

`stop` stops delivering the child's output, sends the ordinary termination signal,
kills the child if it is still running one second later, and waits until it is
reaped. Only the direct child is signalled. `stop` also waits until every stderr
byte read so far has been copied, but not for the end of that stream, which a
descendant of the child can keep open.

When `start` fails before the child runs, for example because the copying thread
cannot be created, the client stays unstarted and no child is left behind.

The client runs on the caller's Tokio local executor, so use it inside a
`tokio::task::LocalSet`. The tasks that read the child's streams and run the
deadlines live there.

## Commands

Each of the 29 session commands is a method, such as `prompt`, `get_state`,
`set_model` or `clone`. `clone` is the session command that duplicates the active
branch; the client itself cannot be cloned. Requests are numbered `req_1`, `req_2`
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

Whichever comes first decides the outcome: the answer, the stop or exit, or the
30 seconds, which count from when the request is sent. A request awaited late still
gets that outcome, and a timeout carries the diagnostic text as it stood at the
deadline. An answer that arrives after the 30 seconds goes to the event callbacks.

`get_last_assistant_text` returns a `Presence<String>`: missing when the session has
no assistant message, and `null` or the text when it answers with one of them.

## Events

`on_event` registers a callback for every record the child writes that answers no
pending request, including records that are not objects. Lines that are not JSON,
and `null`, are ignored. Callbacks run in registration order. Registering the same
callback twice delivers each record twice. A callback added during a delivery
receives that record too. A callback that fails stops the delivery of that record to
the callbacks after it.

`on_event` returns the operation that removes that registration. Calling it again
does nothing; dropping it keeps the registration.

## Waiting for a run

`wait_for_idle` and `collect_events` register when they are called, so an
`agent_end` that arrived before does not count. `wait_for_idle` completes at the next
`agent_end`, and `collect_events` returns every event through it. Both wait 60
seconds unless given a timeout, counted from the call. They fail with
`Timeout waiting for agent to become idle. Stderr: <text>` or
`Timeout collecting events. Stderr: <text>`, and with `Client stopped` when the
child is stopped or exits first. A wait registered while the client is stopping
fails this way when the stop ends. Nothing that arrives after the deadline counts,
and a stop or an `agent_end` before it still counts when the wait is awaited later.

`prompt_and_wait` starts collecting before it sends the prompt, so events the
session writes before its answer are kept. When the prompt fails, the collection
ends with it.

Dropping a request or a wait releases only what it registered; nothing is sent to
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

`crates/maestro-app/examples/rpc_client_demo` is an interactive terminal client
built on these methods:

```sh
cargo run -p maestro-app --example rpc_client_demo -- path/to/maestro
```

# Testing the application session

The application's session tests share one local harness:
[`tests/support/session_harness.rs`](../../crates/maestro-app/tests/support/session_harness.rs)
in `maestro-app`. It is test support inside the crate's integration-test program,
not a published testing library. Extend it only when a test needs something it
lacks, and keep one owner for each helper.

## The harness

`harness(HarnessOptions)` builds a real `AgentSession` through
`AgentSession::new`. The session uses a registered scripted provider, in-memory
credentials, a catalog that knows the scripted models, in-memory settings and
history, and a disposable root that holds the working directory. Options supply
extension factories, tools in place of the built-in ones, skills, prompt
templates, settings and scripted responses. Tests reach the provider through
`h.faux`: they queue responses with `set_responses` and `append_responses`, read
`get_pending_response_count`, and count requests with `state.call_count()`.

No test calls a real provider, uses a real key, reaches the network or spends
tokens. Every response comes from the scripted provider or a stream the test
installs.

## Reading what the session did

- `get_message_text` returns a message's text: string content unchanged, or its
  text blocks joined with one newline each. Thinking, images and tool calls add
  nothing, and a record without content has no text.
- `get_user_texts` and `get_assistant_texts` apply that projection to the user
  or assistant messages of the session history, in order.
- `message_text` is a different projection. It joins text blocks with no
  separator and falls back to a summary, matching the recorded cases that use it.

Stored entries come from `h.session.session_manager()`. Extension handlers read
the same history through their context's `session_manager()`.

## Waiting without clocks

Tests never sleep or poll. They wait on signals the session or the scripted
provider sends:

- A scripted answer can notify a `Notify` when the provider is asked and then
  wait for the request's own cancellation, so `abort` reaches the signal the
  provider received.
- A handler can report that it has started, then wait on a `watch` channel until
  the test releases it.
- `waiting_tool` reports when its execution starts and answers once the test
  releases it, so a test queues messages while the run is held and observes the
  queue mirrors before delivery.
- `session_run_ended` notifies once the session delivers the run's `agent_end` to
  its listeners. The agent becoming idle alone does not show that the session has
  finished processing its queued events.

`prompt_to_end` uses `session_run_ended` to wait for prompt completion, including
session persistence. Both helpers live in the harness; concurrency and navigation
checks use that same completion route. The harness also uses the SDK's
`transform_context` adapter to deliver extension context hooks.

Registered callbacks hold notifiers and recorders, not the session, and no
borrow outlives a callback or an await.

## Fixtures

Recorded cases live in `tests/fixtures/agent_session/*.json`. The concurrency
checks compare each whole observation with its case in `concurrent.json`, so a
recorded field no test reads fails them. The prompt and queue checks read one
operation record per test name from `prompt_queue.json` and compare it whole:
history, queue mirrors, provider requests, tool runs, errors and count fields
included. The model, extension, statistics, reload, navigation and traversal
checks each compare their complete keyed observation from `model_navigation.json`.
`assistant-message-with-thinking-code.json` keeps only the content of a long
thinking and code answer, signature included; the scripted provider owns the
rest of the message. The `empty-agent` and `empty-cwd` roots are copied into
disposable storage before a session uses them, in a child process whose home
directory is also disposable. Tests never write into committed fixtures.

## Cleanup

`AgentSession::dispose` makes contexts taken from the session stale. Dropping
the harness unregisters the scripted provider and removes the disposable root.

## Running the session tests

Run one group of checks through the shared integration-test program:

```sh
cargo nextest run -p maestro-app --locked --test integration \
  -E 'test(/^agent_session_concurrent_checks::/)'
```

The model, reload and navigation qualification group includes the concurrency
regressions that share its completion helpers:

```sh
cargo nextest run -p maestro-app --locked --test integration \
  -E 'test(/^(agent_session_(model_extension|stats|tree_navigation|concurrent)|reload_stale_resource_settings|tree_cancel_compacting|tree_traversal)_checks::/)'
```

The prompt and queue checks run together:

```sh
cargo nextest run -p maestro-app --locked --test integration \
  -E 'test(/^(agent_session_prompt_checks|agent_session_queue_checks|queued_slash_command_followup_checks)::/)'
```

# Native extension hooks

Register callbacks with [`ExtensionAPI::on`](registration.md). The runner visits
extensions in load order and callbacks in registration order, including callbacks
appended while the active dispatch awaits another callback. Each dispatch creates
one [context](registration.md#contexts); no registration guard is held during a callback.

`emit` handles notifications and session-before events. Notifications ignore
returned values. Session-before events retain the last supplied result and stop
only when its cancellation field is explicitly true. The shutdown helper reports
whether shutdown handlers existed; it does not tear down the runner.

Dedicated routes preserve their own outcomes:

- Context hooks start with independent message payload copies, then share the
  evolving messages. Returned arrays replace them, including empty arrays.
- Provider-request hooks replace the payload on any supplied JSON value, including
  null, false and empty collections. Returning no result leaves it unchanged.
- Before-agent hooks accumulate custom messages and chain system prompts, including
  empty prompts. Retained contexts keep their invocation's final prompt; ordinary
  contexts continue reading the bound application action. Both remain guarded
  against stale use.
- Message-end hooks accept only same-role replacements. Tool-result hooks merge
  supplied content, details and error fields; an empty patch is not a change.
- Input hooks chain transforms and stop on `Handled`. Missing or null replacement
  images retain the current images; an explicit list, even empty or equal, marks
  image replacement. `Continue` preserves mutable image edits without declaring a
  transform. Text changed and restored can finish as `Continue`. The event carries
  `streamingBehavior` (`steer` or `followUp`) only when the input arrives while a
  response streams and the caller chose a delivery; idle input carries none.

An extension's `abort` goes to the abort handler the host bound with the session's
extensions. The interactive mode binds one that does what the interrupt key does: it moves
every queued message back into the editor exactly as typed, then stops the running turn,
whether or not a tool confirmation is open. Without a bound handler, the session aborts its
own run.

Tool-call hooks retain the last nonblocking result and stop on an explicit block.
Their errors propagate unchanged, without error-listener delivery. Edited arguments
reach the caller without another validation pass. User-shell hooks return the first
supplied result, retaining caller-owned callable operations. Resource discovery
keeps separate ordered path lists, duplicates and the current emitting extension
path, without normalization.

Other callback failures are delivered to the runner's error listeners and dispatch
continues. A listener failure propagates and stops dispatch. Native mutable operand
edits are not rolled back on callback failure. Authored diagnostic stacks survive;
engine stack text is not manufactured. A changed-role replacement reports exactly
`message_end handlers must return a message with the same role`.

Artifact discovery and loading are not supplied by this hook runner. They are a
separate [delivery](https://github.com/Orchestration-Maestro/maestro/issues/206).

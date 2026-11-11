# Extension context lifecycle

Use the context passed to the current callback. For registration and runnable
examples, see [owned registration](registration.md#command-and-replacement-contexts).

## Reads and bindings

A base context reads the runner's current UI and bound operations when called.
Its session history, catalog and working directory come from the supplied
`ExtensionRunnerSession`. The model reader is captured at context creation,
but calling that reader still obtains its current result. Command contexts keep
these base reads and add session-control operations.

Command operations check activity and select their bound callback when called,
before polling the returned future. Rebinding or invalidating the runner does
not retarget an operation already started. Unbound waits and reloads do nothing;
unbound session changes return `cancelled: false` without running setup or
continuation callbacks. These are domain defaults, not evidence that the
application has changed sessions.

Invalidation makes guarded calls return a catchable stale-context error. The
first invalidation message wins, including an empty message. Catching that error
does not invalidate an independent runner. Values obtained earlier, such as a UI
handle or session history, remain the original objects; they are not revoked or
redirected to a replacement session.

## Continue in the replacement

`new_session`, `fork` and `switch_session` accept a `with_session` callback.
The application operation supplies its `ReplacedSessionContext`. Capture plain
data for that callback and use its fresh context for post-replacement work:
[message continuation example](registration.md#continue-with-the-fresh-context).
The callback still executes its original closure; it does not acquire new
extension state. Do not rely on captured state that shutdown may have cleared.

A replacement context inherits guarded command operations and adds custom- and
user-message callbacks. Those two message callbacks keep reaching their supplied
session even after the runner is invalidated. Message content, details and
options pass through unchanged, including missing versus null details and absent
versus empty options.

The new-session `setup` callback receives writable session history. See the
[history setup example](registration.md#write-history-during-setup). The context
wrapper forwards setup and continuation callbacks; it does not run them or own
replacement ordering.

A setup failure stops its continuation, and a continuation failure leaves the
effects it already completed in place; both reach the awaiting command unchanged.

Reusing a captured command context after successful replacement is incorrect:
[stale capture example](registration.md#avoid-captured-contexts). Previously
extracted raw history is still old history, not a route to the new session.

## Host phases around a replacement

The application owns these phases. The component adapter is qualified against a
host that runs a replacement in this order. Extensions hear `session_before_switch`
(new and switch) or `session_before_fork` and may veto. The old generation then
hears `session_shutdown`, the host releases the old UI and invalidates the old
contexts, and the host activates a fresh generation, binds it and announces
`session_start`. Setup and the continuation run last, against the fresh
generation. The old command frame resumes after all of it.

- A veto finishes the operation with `cancelled: true`. No shutdown, setup,
  continuation, fresh generation or invalidation follows, and the old context
  stays valid. A veto cancels only its own operation: a fork veto does not stop
  a new session. Reload has no veto event.
- Shutdown handlers fail independently. A failing handler is reported with its
  extension and event name, the remaining handlers still run, and invalidation
  still follows.
- A failing phase ends the operation with that phase's error. Nothing after it
  runs and nothing before it is rolled back. A failure before invalidation
  (preparation, shutdown, UI release) leaves the old context valid; a later one
  (activation, setup, continuation) leaves it stale.

## Reload ends the old handler

Reload takes no arguments and has no `with_session` callback. Return after
awaiting it, as in the [reload example](registration.md#reload-and-return).

Reload captures the flag values, announces `session_shutdown` with reason `reload`
and invalidates the old contexts. It then reloads settings, resets providers and
reloads resources, activates a fresh generation with the captured flags restored,
announces `session_start` and `resources_discover` with reason `reload`, and
resumes the old command frame. That frame's context does not become fresh again:
its guarded reads reject as stale. Previously obtained raw objects remain usable
under their own ownership.

The application's reload does not invalidate the old contexts yet, so a frame that
awaits it can still read through its old context. Issue #4397 tracks the change.

## Actions while loading

A factory that calls an action before the runtime binds it gets a catchable error:
`The extension runtime is not initialized yet. Do not call action methods while
extensions are loading.` for messages, entries, session names, tool selection
and thinking level, and `The extension runtime is not initialized yet` for a
model selection.
The call never reaches a host operation and is not queued. Catch the error and
keep registering; the factory's other work completes.

## A failed activation keeps unrelated providers

Provider registrations a factory made while loading apply when the host binds the
runtime. If the host's registration operation fails for one provider, that failure
is reported to the error listeners as a `register_provider` error attributed to the
extension. The providers registered before and after it stay registered, and the
queue is empty afterwards. A later registration that fails reports its own error
to its caller and leaves every earlier provider in place.

## Application and component boundaries

The extension domain supplies owned contexts, activity checks and callback
forwarding. Actual preparation, vetoes, teardown, rebinding, startup, setup and
continuation scheduling belong to application session operations. Their delivery
is tracked in [application lifecycle](https://github.com/Orchestration-Maestro/maestro/issues/226)
and [lifecycle integration tests](https://github.com/Orchestration-Maestro/maestro/issues/230).
Domain callback tests do not qualify overlapping application replacements or
which session an application continuation selects after an await.

[Path loading and discovery](loading.md) take the module loader from the caller,
so the domain never evaluates a module itself.

Component execution, faults, resource disposal and pending stream work belong to
the [component runtime](https://github.com/Orchestration-Maestro/maestro/issues/206).
The [component execution guide](native-execution.md) lists which retained reads,
views and replacement callbacks stay usable past their originating callback.
Application UI integration and component host bindings remain with the
[UI integration](https://github.com/Orchestration-Maestro/maestro/issues/203)
and [host bindings](https://github.com/Orchestration-Maestro/maestro/issues/210)
deliveries. The domain context contract does not assert those engine guarantees.

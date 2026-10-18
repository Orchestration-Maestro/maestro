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

Reusing a captured command context after successful replacement is incorrect:
[stale capture example](registration.md#avoid-captured-contexts). Previously
extracted raw history is still old history, not a route to the new session.

## Reload ends the old handler

Reload takes no arguments. Return after awaiting it, as in the
[reload example](registration.md#reload-and-return). An old command frame can
resume after reload, but its context does not become fresh again. Previously
obtained raw objects remain usable under their own ownership.

## Application and component boundaries

The extension domain supplies owned contexts, activity checks and callback
forwarding. Actual preparation, vetoes, teardown, rebinding, startup, setup and
continuation scheduling belong to application session operations. Their delivery
is tracked in [application lifecycle](https://github.com/Orchestration-Maestro/maestro/issues/226)
and [lifecycle integration tests](https://github.com/Orchestration-Maestro/maestro/issues/230).
Domain callback tests do not qualify overlapping application replacements or
which session an application continuation selects after an await.

Component execution, faults, resource disposal and pending stream work belong to
the [component runtime](https://github.com/Orchestration-Maestro/maestro/issues/206)
and its [retention tests](https://github.com/Orchestration-Maestro/maestro/issues/209).
Application UI integration and component host bindings remain with the
[UI integration](https://github.com/Orchestration-Maestro/maestro/issues/203)
and [host bindings](https://github.com/Orchestration-Maestro/maestro/issues/210)
deliveries. The domain context contract does not assert those engine guarantees.

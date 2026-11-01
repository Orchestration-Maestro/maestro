# Agent execution affinity

## Decision

UI-entering interactive work uses a caller-owned local executor. Local callbacks
and their futures may retain single-thread UI objects, including the editor and
live theme. The embedding SDK defaults to thread-safe callables on native targets and
browser-local `Rc` callables on browser targets; independent native I/O retains
its existing execution behavior.

The sealed `Local` and `Threaded` representations belong to `maestro-models`, the
lowest existing callback owner. They select future, shared and weak pointer types.
Models owns the provider callable, payload/response hooks, options family and
provider submission representation. One retained callable supports either family;
its return is associated with the invocation family. The same mechanism serves
provider callbacks and scripted factories. `maestro-agent` owns agent callback
signatures, conversion from its existing task admission and typed local entry.
Both representations use one loop, admission path, listener traversal, progress
FIFO and lifecycle. Extension command, shortcut, completion and event-bus
callbacks select the same families through `maestro-extensions`'
`ExtensionExecution`; one registration owner, query algorithm and bus serve
both. The base and command contexts are concrete single-thread
`ExtensionContext` and `ExtensionCommandContext` handles. A
thread-safe command or shortcut callback can read its context only before it suspends; a
local callback may keep it. Message renderers also
follow the family, stored per family without their details type. The extension
UI handle and its factories are always local, and so is every runner, whatever
its callback family; registrations, the API facade and default callbacks keep
their thread-safe defaults. The callback families add no crate or dependency
edge.

## Trade-off

A local run cannot be submitted to a thread-safe task facility. Its caller must
supply a local task facility instead. In return, synchronous UI callbacks operate
on the real UI objects without a general dispatcher, thread-local registry or
unsafe thread-safety assertion. Component execution alone uses typed requests
between local domain owners and the thread-safe component Store. The Store keeps
request endpoints and resource identities, not local contexts or UI objects.
Synchronous callbacks pump these requests on the calling thread; nested component
calls use the active Store accessor. Typed aliases retain existing SDK
struct-literal and explicit message-type call syntax.

Payload and response hooks follow the invocation family: local hooks and futures
may retain single-thread objects, while default SDK hooks remain thread-safe on
native targets. The registered native callable itself remains thread-safe and
accepts either family without caller-side mode matching. Local registered dispatch
requires its caller's submission facility in the entry type; a replacement-stream
entry requires none. Providers retain their existing default submission policy
when a threaded caller supplies no facility.


## Definition ownership and invocation

Callback Send/Sync governs captures, not invocation operands or futures. Local
registered invocation permits retaining the real context without weakening the
thread-safe facade or default Threaded SDK invocation. Runner and context
factories stay outside registration storage. Renderer operands and state stay
local; retained state types are owned. Family-aware renderer handles and explicit
invocation typing reuse the existing registration, wrapper and execution owners.

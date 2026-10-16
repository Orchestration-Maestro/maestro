# Agent execution affinity

## Decision

UI-entering interactive work uses a caller-owned local executor. Local callbacks
and their futures may retain single-thread UI objects, including the editor and
live theme. The embedding SDK defaults to thread-safe callables; independent
native I/O retains its existing execution behavior.

The sealed `Local` and `Threaded` representations belong to `maestro-models`, the
lowest existing callback owner. They select future, shared and weak pointer types.
`maestro-agent` owns the agent callback signatures and concrete submission glue.
Both representations use one loop, admission path, listener traversal, progress
FIFO and lifecycle. No crate or dependency edge is added.

## Trade-off

A local run cannot be submitted to a thread-safe task facility. Its caller must
supply a local task facility instead. In return, synchronous UI callbacks operate
on the real UI objects without a dispatcher, thread-local registry or unsafe
thread-safety assertion. Typed aliases retain existing SDK struct-literal and
explicit message-type call syntax.

Model payload and response callbacks retain their existing thread-safe contract;
local agent execution alone does not make those provider callbacks local.

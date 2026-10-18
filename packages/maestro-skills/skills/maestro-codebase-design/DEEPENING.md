# Deepen a cluster at its seam

Classify dependencies before choosing how the resulting module is tested:

- **In-process:** pure computation or memory; test the deepened interface directly.
- **Local-substitutable:** a local test stand-in exists; run it behind the internal seam.
- **Remote but owned:** inject a port at the network seam, with production transport and an in-memory test adapter.
- **True external:** inject the third-party service at a port and use a controlled mock adapter in tests.

One adapter alone does not justify a new port. Keep internal test seams private; the external interface should
not grow merely to expose implementation collaborators.

Replace, do not layer: when new behavioural tests at the deepened interface protect the old guarantees, remove
obsolete shallow tests. Preserve guarantees before deleting observations. Assert caller-visible outcomes,
not private state or call chains. A structural rewrite should not require rewriting the behaviour contract.

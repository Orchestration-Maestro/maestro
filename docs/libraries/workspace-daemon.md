# Workspace daemon libraries

`maestro-ipc` uses `interprocess =2.4.2` on Unix with default features disabled.
No Tokio or asynchronous feature is enabled. It uses the Apache-2.0 licence option.

The locked dependency closure is:

| Package | Version | Licence |
|---|---|---|
| interprocess | 2.4.2 | Apache-2.0 |
| doctest-file | 1.1.1 | 0BSD |
| libc | 0.2.190 | MIT OR Apache-2.0 |
| recvmsg | 1.0.0 | 0BSD |
| widestring | 1.2.1 | MIT OR Apache-2.0 |
| windows-sys | 0.61.2 | MIT OR Apache-2.0 |
| windows-link | 0.2.1 | MIT |

`recvmsg`, `widestring`, `windows-sys` and `windows-link` are inactive Windows
branches of this closure. Exact 0BSD permissions for doctest-file and recvmsg are
recorded in the shared-CI configuration. The leaf adds no production dependency
on another domain crate. Native tests use the existing checked test-process owner;
the native feature-unifier dependency remains API-free.

See [workspace sockets](../workspace-daemon.md#sockets) for the public operations.

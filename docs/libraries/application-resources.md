# Application resource libraries

Resource loading and session services add no new package. `maestro-app` gains
its allowed native edge to `maestro-theme` for theme files and theme directories.

The existing native Tokio `=1.53.3` dependency enables `sync` for the channel
that carries threaded event bus continuations to the caller's task set. The
existing chrono `=0.4.45` dependency enables `clock` on native targets, which the
system prompt uses for the local date. Tokio is MIT; chrono is MIT or Apache-2.0.
Browser builds do not include either feature. See
[application resources](../application/resources.md).

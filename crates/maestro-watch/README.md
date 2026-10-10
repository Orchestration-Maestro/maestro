# Filesystem-watch effects

`fs_watch` provides replaceable directory-watch and one-shot timer effects.
Feature owners decide filtering, reload and retry policy. The native adapter uses
a caller-driven local task set; browsers provide their own `WatchOperations`.

See [filesystem-watch documentation](../../docs/filesystem-watch.md) for the
helpers and native handle lifecycle.

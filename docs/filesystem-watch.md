# Filesystem-watch effects

`maestro_watch::fs_watch` owns `FsWatcher`, `WatchTimer` and `WatchOperations`.
Consumers supply the operations and retain the returned handles; filtering,
debounce, reload and retry policy belong to the consumer. The shared
`FS_WATCH_RETRY_DELAY_MS` constant is 5000 milliseconds and schedules nothing.

`close_watcher` ignores an absent handle and suppresses a close error.
`watch_with_error_handler` returns a successfully created handle; creation failure
calls the error callback synchronously and returns absence. Later watch errors
reach the registered callback without an automatic close or retry.

Outside browsers, `NativeWatchOperations::new` takes the caller's
`Rc<tokio::task::LocalSet>`. The caller drives it inside a timer-enabled runtime.
The adapter anchors the directory once and registers a non-recursive watch.
Closing or dropping a watch stops its pending dispatch, including remaining paths
in a notification when closed inside a callback. Cancelling or dropping a timer
prevents its pending callback, without cancelling other handles.

The native adapter ignores access events, projects other paths to relative names
in their received order, and reports unknown names for rescan events, missing
paths or paths outside the directory. It does not normalize authored path strings
at the shared interface. Browsers provide their own `WatchOperations`.

The existing theme watch imports are same-type aliases of this owner; see
[theme reload policy](theme.md#watching-and-reload).

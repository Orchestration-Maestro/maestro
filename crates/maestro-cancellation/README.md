# Shared cancellation

`Cancellation` owns cooperative cancellation independently of request observers.
Cloned handles share the aborted flag. The first `abort` marks the signal and
notifies its observers; later calls leave it marked. `cancelled` returns an owned
future that can outlive its originating handle. Dropping a future does not abort
work.

```rust
use maestro_cancellation::Cancellation;

let signal = Cancellation::new();
let retained = signal.clone();
signal.abort();
assert!(retained.is_aborted());
```

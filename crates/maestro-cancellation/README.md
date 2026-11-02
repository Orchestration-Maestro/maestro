# Shared cancellation

`Cancellation` owns cooperative cancellation independently of request observers.
Cloned handles share the aborted flag. The first `abort` marks the signal and
notifies its waiters; later calls leave it marked. `cancelled` returns an owned
future that can outlive its originating handle. Dropping a future does not abort
work.

```rust
use maestro_cancellation::Cancellation;

let signal = Cancellation::new();
let retained = signal.clone();
signal.abort();
assert!(retained.is_aborted());
```

## Abort observers

`subscribe` registers a callback that runs once, inside the `abort` call that
reaches it, before `abort` returns. Observers run in registration order and the
same callback is admitted once. An observer may abort again, which does nothing,
or register and remove observers; the running walk sees those changes. A
registration made after the abort never runs.

`subscribe` returns the removal operation. Calling it removes the observer and
later calls do nothing; dropping it leaves the observer registered.

```rust
use maestro_cancellation::Cancellation;
use std::{cell::Cell, rc::Rc};

let signal = Cancellation::new();
let runs = Rc::new(Cell::new(0));
let counter = Rc::clone(&runs);
let _remove = signal.subscribe(Rc::new(move || counter.set(counter.get() + 1)));
signal.abort();
signal.abort();
assert_eq!(runs.get(), 1);
```

Observers stay on the thread that registered them, because the signal crosses
threads and they cannot. An abort on another thread wakes the waiters of
`cancelled`; the owning thread runs its observers when it calls `abort` itself,
usually from such a waiter.

# File-lock acquisition

`maestro_lock::acquire` returns the supplied file after acquiring an exclusive lock.
It makes at most ten attempts, waiting 20 ms only between contended attempts.
The final contention error or the first non-contention error is returned unchanged.
The waits are requested delays, not a wall-clock completion deadline.

Callers own opening, permissions, persistence and unlocking. Settings and credentials
delegate synchronous acquisition here; asynchronous credential acquisition remains
with [credentials](credentials.md). Browser compilation does not provide browser
file locking.

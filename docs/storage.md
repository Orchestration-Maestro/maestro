# Record storage

`maestro-storage` stores session records behind two synchronous, object-safe
interfaces: `Storage` owns the collection; `RecordSession` owns a handle permanently
bound to one session identity. No model, transcript or tree interpretation lives
in storage. Header and record bytes are opaque caller data, including empty or
non-text bytes. Callers supply IDs and encoding; storage generates neither IDs
nor timestamps.

## Configuration and memory lifetime

Construct `MemoryStorage::new()` explicitly and supply a `SessionHeader` with an
identity and header bytes. There are no settings, timers, filesystem paths,
background workers or persistence fallbacks. An empty identity is valid caller
data. Duplicate session creation is rejected without replacing existing state.

Memory always reports `persistent_locator: None` and `resumable: false`.
Closing a handle retains records within the same adapter. Opening that identity
creates a new, independent handle. A fresh adapter cannot open it: reopening
memory is not restart persistence. Adapter locators, when supplied by other
adapters, are opaque and need not denote files. Listing includes closed sessions
and promises no sorting or filtering policy.

## Consistency and lifecycle

An append batch and its explicit selected position publish as one atomic state.
Record IDs must be unique within a session and within a batch. A selected ID must
exist after the proposed append. `None` selects the position before records;
empty batches are valid position updates. `select` changes only the position.
These checks validate references, not parent relationships or a session tree.

Independent opens serialize mutations against the same session state, without a
FIFO scheduling promise. Reads and ID lookups return fully detached values;
changing their nested byte buffers or metadata cannot mutate stored state.

Operations block synchronously. `close` stops this handle's admission before
waiting for all its admitted writes to settle. Newly submitted reads and mutations
fail `Closed`. Repeated close succeeds. `Arc` clones share admission; independent
opens remain usable. Closing one handle neither closes another nor erases records.

## Errors

- `Rejected { reason }`: the operation definitely made no mutation, including
  duplicate records or missing selected IDs. The previous whole snapshot remains
  unchanged.
- `Uncertain { reason }`: a mutation's outcome is unknown, not a rejection or
  rollback claim. An adapter must disable subsequent and pending mutations on
  that handle and its clones before returning it. Close remains available.
  Automatic retries, recovery and reconciliation/reopen after uncertainty are
  not part of this interface. Memory does not inject uncertain outcomes.
- `Closed`: this handle is closing or closed.

Reasons are diagnostic text, not machine-readable codes or payload dumps.
`StorageError` implements `Display` and `std::error::Error`.

## Example

```rust
use maestro_storage::{MemoryStorage, Record, SessionHeader, Storage};

let storage = MemoryStorage::new();
let handle = storage.create(SessionHeader {
    session_id: "conversation".into(),
    data: vec![0, 255],
})?;
handle.append(vec![Record {
    id: "first".into(),
    data: b"caller encoding".to_vec(),
}], Some("first".into()))?;
let snapshot = handle.read()?;
assert_eq!(snapshot.selected_position.as_deref(), Some("first"));
assert_eq!(handle.get("first")?, Some(snapshot.records[0].clone()));
assert!(!snapshot.metadata.resumable);
handle.close()?;
let reopened = storage.open("conversation")?;
assert_eq!(reopened.read()?, snapshot);
# Ok::<(), maestro_storage::StorageError>(())
```

## Adapter conformance

The owning crate unconditionally publishes `conformance` assertion functions for
adapter test suites. Each takes a fresh isolated `&dyn Storage`, so the same
caller works with memory or another adapter. Contract violations intentionally
panic; these assertions are not production request handling.

```rust
use maestro_storage::{conformance, MemoryStorage, Storage};

fn qualify_create(storage: &dyn Storage) {
    conformance::create_append_read_open(storage);
}
qualify_create(&MemoryStorage::new());
// Another adapter's fresh store can be passed to the unchanged function.
```

Run the shared data, rejection, isolation, detached ownership and serial mutation
cases for each adapter. `Controls` coordinates real admitted/staged writes and
close admission/draining through channels, without replacing behavior or holding
the snapshot lock at a pause. Memory scheduling hooks exist only in private unit
tests; no production memory fault or scheduling switches are exposed. A separate
controlled integration-test adapter uses its own record state and supplies both
before-publication and after-publication uncertainty. Pending mutations cannot
proceed past either uncertain outcome.

These memory and controlled tests establish record-interface behavior, not disk
durability, crash recovery, restart qualification, fork or context projection.

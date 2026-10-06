# Preferences

`maestro-settings` owns accepted global and project preferences through
`SettingsManager`. `Settings` is a raw JSON value, not a resolver. Storage,
execution and file locations are supplied by the caller. The crate does not
install packages, discover resources, select models or contact telemetry services.

## Raw and typed reads

`get_global_settings` and `get_project_settings` return detached raw snapshots.
Unknown keys and wrong-typed values remain present. Each typed getter treats a
wrong JSON type as unset and applies its own fallback; reads do not write defaults.
Collection reads are owned copies. Change preferences through setters.
The key `__proto__` is an ordinary unknown key, not a fallback for typed reads.

Settings parsing has no fixed nesting limit. Native recursive processing grows
its stack as needed; manager-owned trees are torn down iteratively. Raw snapshots
remain ordinary `serde_json::Value` values: callers handling very deep snapshots
must also dismantle objects and arrays iteratively before dropping them, rather
than relying on the value's recursive destructor. Two-space pretty output is
retained at every depth. Very deep saves are bounded by available memory, since
indentation grows quadratically for a single-child chain and storage receives
one complete output string.
Package object and warning records retain their original `property_order` through
typed getter/setter round-trips; additional properties stay in `extra`. New records
can use an empty property order to serialize their supplied fields in insertion
order.

Objects and arrays are accepted roots. Primitive roots, malformed JSON, numeric
overflow and lone UTF-16 surrogate escapes produce scoped load errors. Missing or
zero-byte text is empty; whitespace-only text is not. A leading byte-order mark
is not stripped. Nonfinite numeric setter values remain available to typed reads
where the getter permits them; JSON snapshots and writes represent them as null.
Named array properties are available to typed reads but absent from raw snapshots.
String object spreads enumerate UTF-16 code units; a surrogate unit becomes the
Unicode replacement character in the resulting JSON member.

## Precedence

Project preferences override global preferences. A pair of ordinary objects
spreads one level: immediate siblings survive, but grandchildren are replaced.
Arrays, primitives and null replace. Runtime `apply_overrides` uses the same merge
and never persists. It returns an immediate error for a null root, leaving both
accepted and effective values unchanged. Every setter republishes from accepted scopes, discarding all
runtime overrides.

For example, global `retry.provider = {timeoutMs: 10, maxRetries: 2}` plus project
`retry.provider = {maxRetries: 4}` yields only the project provider object.

## Defaults

| Preference | Getter fallback |
| --- | --- |
| Provider, model, thinking, theme, changelog, session directory, shell fields, npm command, enabled models | Absent |
| Steering and follow-up modes | `one-at-a-time`; empty strings also fall back |
| Transport | `auto`; an empty string is retained |
| Compaction | Enabled; reserve 16384; keep recent 20000 |
| Branch summary | Reserve 16384; skip prompt false |
| Application retry | Enabled; retries 3; base delay 2000ms |
| Provider retry | Timeout/retries absent; maximum retry delay 60000ms |
| Hide thinking, quiet startup, collapse changelog | False |
| Install telemetry, skill commands | True |
| Packages, extensions, skills, prompts, themes | Empty ordered lists |
| Show images, automatic image resize | True |
| Terminal progress, block images | False |
| Image width | 60; finite loaded values floor to at least 1 |
| Editor padding, autocomplete limit | 0 and 5; loaded fractions remain unchanged |
| Double escape, tree filter, code indent | `tree`, `default`, two spaces |
| Thinking budgets, warnings | Absent budgets; empty warning record |

Only tree filters validate their known strings: `default`, `no-tools`,
`user-only`, `labeled-only`, `all`. Other string choices remain open data.
`MAESTRO_CLEAR_ON_SHRINK=1` and `MAESTRO_HARDWARE_CURSOR=1` provide environment
fallbacks. Explicit false overrides both. Clear-on-shrink null is false without
an environment fallback; hardware cursor null uses its fallback.

Session directory text is literal except exact `~` and `~/`, which expand the
host home directory using lexical host join. On Unix, a set `HOME` wins even
when empty; an unset `HOME` uses the account database. Windows uses `USERPROFILE`
with the host profile lookup fallback. No whitespace is trimmed.
Numeric setters floor and clamp width to at least 1, padding to 0–3 and
an autocomplete limit to 3–20. Their arithmetic preserves NaN. Eight nested
setters return immediate assignment errors for truthy primitive containers;
falsy containers become objects. Storage failures are drained, not returned by
setters.

## Publication and persistence

The executor must defer polling until after the synchronous setter returns.
It must drive queued jobs without `flush`; a current-thread event-loop executor
or a controlled deferred scheduler satisfies this contract. Manager clones share
one queue across both scopes. `flush` only observes a captured queue tail.

A write captures accepted values and modified top-level/immediate nested keys.
It reads fresh raw storage, applies the existing key conversions and replaces
only those captured keys. External siblings survive on disk without appearing
in the cache until reload. Successful jobs clear their scope's current tracking;
failed jobs retain tracking and append the original scoped error. Later jobs
continue. JSON writes use two-space indentation, no final newline, integer-key
ordering and binary64 number spelling.

`reload` waits for pending writes, loads global then project independently,
retains each failed scope and clears tracking and runtime overrides. Failed loads
latch persistence off for that scope. Setters still change accepted memory, but
repairing bytes or draining errors alone does not unlatch it. A successful reload
unlatches without replaying session-only changes. `drain_errors` returns original
error objects in observation order and empties the error list.

The existing stored-key conversions cover queue mode, websocket transport,
skill directory records and the provider retry delay. They run on seed, load
and fresh write data; there is no whole-document admission schema.

## Replaceable storage

`SettingsStorage::with_lock` receives a synchronous raw-text callback. Returning
`None` preserves bytes; returning `Some` replaces them even with empty text.
Memory callbacks run outside the text mutex and can re-enter either scope. They
observe a captured text value; a returned replacement is published after the
callback returns, so an outer replacement wins a nested write to the same scope.
A callback panic leaves the memory adapter usable without publishing an outer
replacement; any completed nested writes remain visible.
No caller callback, waker lifecycle or executor invocation runs under a manager
state mutex.
Loads capture raw text in the callback and parse only after storage completes
successfully, so a storage completion failure takes precedence over malformed text.
The memory adapter and arbitrary caller adapters work on native and browser
builds. `FileSettingsStorage` and `SettingsManager::create` are native-only.

The file adapter joins supplied locations with `settings.json` and uses lexical
absolute `.lock` directory identities without resolving symlink targets.
Existing files lock before reading. A missing-file callback runs before directory
creation and locking; writing then locks without rereading. Lock contention has
ten attempts with nine 20ms gaps. The lease uses a 10000ms stale threshold and
5000ms heartbeat schedule; synchronous callbacks do not yield to timers. Errors
release acquired locks, with release failures taking precedence. Writes are
in-place: partial bytes may remain after failure. There is no rename, fsync or
rollback guarantee. UTF-8 file decoding replaces malformed byte sequences.
No operation prints to the console.

## Deferred memory example

```rust
use maestro_settings::{InMemorySettingsStorage, SettingsManager};
use std::{future::Future, pin::Pin, sync::{Arc, Mutex}, task::{Context, Poll, Waker}};

type Job = Pin<Box<dyn Future<Output = ()> + Send>>;
let jobs = Arc::new(Mutex::new(Vec::<Job>::new()));
let deferred = jobs.clone();
let manager = SettingsManager::from_storage(
    Arc::new(InMemorySettingsStorage::new()),
    Arc::new(move |job| deferred.lock().unwrap().push(job)),
);
manager.set_theme("dark".into());
assert_eq!(manager.get_theme().as_deref(), Some("dark"));
assert_eq!(jobs.lock().unwrap().len(), 1);

let mut context = Context::from_waker(Waker::noop());
let mut write = jobs.lock().unwrap().pop().unwrap();
assert!(matches!(write.as_mut().poll(&mut context), Poll::Ready(())));
let mut flush = std::pin::pin!(manager.flush());
assert!(matches!(flush.as_mut().poll(&mut context), Poll::Ready(())));
assert!(manager.drain_errors().is_empty());
```

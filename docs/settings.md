# Settings

`maestro-settings` owns effective settings resolution, origins and manifest value
locks. It consumes the manifest's settings section, not a manifest envelope.
Unknown keys remain data. Resource lists are not activated or discovered here.

## Resolution and defaults

Objects merge recursively in engine → manifest → user → project order. Scalars,
null and arrays replace; null is present, not deletion. Omitted keys preserve
lower-layer values. Resource arrays replace rather than append.

Defaults are defined once by the engine:

| Key | Default |
| --- | --- |
| `packages` | `[]` |
| `extensions` | `[]` |
| `skills` | `[]` |
| `promptTemplates` | `[]` |
| `enableSkillCommands` | `true` |
| `quietStartup` | `false` |
| `hideThinking` | `false` |
| `collapseChangelog` | `false` |

Supplied engine values override these defaults before manifest application.
Neither personal configuration nor unrelated domain defaults are read.

For example:

```text
engine:   {"display":{"theme":"light","width":80},"packages":["base"]}
manifest: {"display":{"width":100},"endpoint":"governed"}
locks:    [["endpoint"]]
user:     {"display":{"theme":"dark"},"packages":["local"]}
project:  {"display":{"width":120}}
effective:{"display":{"theme":"dark","width":120},
           "endpoint":"governed","packages":["local"], ...engine defaults}
```

`resolve()` returns a detached snapshot. `origins` has one entry for every scalar,
null, whole array and empty object. Nonempty objects have descendant entries, so
`display.theme` is User and `display.width` is Project in this example. Replacing
an object with a scalar removes its old descendant origins.

`read_scope(SettingsScope::User)` and `read_scope(SettingsScope::Project)` return
cloned accepted maps, without I/O. Shadowed user values remain readable. These
maps include session-only stored edits, but exclude defaults, other layers,
runtime overrides and unrequested external edits. Mutating a returned nested
value cannot change the manager.

## Updates and locks

All callers use `Settings::set(target, path, value)`. Paths are lists of literal
object-property names: `["a.b"]` differs from `["a", "b"]`. Slash, tilde and the
empty string are ordinary property characters. Arrays cannot be traversed.
Missing object ancestors can be created; existing nonobjects cannot be traversed.
An empty setter path replaces the root and requires an object. A subtree edit
replaces that subtree exactly, not a recursive patch.

Only `ManifestSettings.locks` grants lock authority. Lock paths must be nonempty
and target an existing engine-plus-manifest value before stored layers apply.
Null and whole arrays can be locked. A subtree lock freezes membership as well
as values. Duplicates and overlapping locks are harmless. Ordinary settings
properties named `locks` cannot grant, remove or retarget governance.

Equal restatement is allowed and changes the winning value origin, not Manifest
lock authority. Each layer is checked before the next layer; project values
cannot conceal a user conflict. Ancestor/root replacements omitting a locked
child fail even when lower-layer fallback could restore it. Descendant edits
cannot change a locked subtree. Logical rejection changes neither the accepted snapshot
nor stored settings. Native I/O failures retain the snapshot but can leave partial
file bytes.

`Stored(User)` and `Stored(Project)` validate two independent candidates: the
cached target plus the requested edit for publication, and the fresh transaction
map plus that same edit for persistence. Both enforce ancestor omission and
per-layer locks against the baseline and cached other scope. Saving does not read
the other scope. Disk preserves unrelated external keys; cached/effective views
publish only the requested change until explicit reload. Successful stored edits
discard old overrides. A load-failed scope instead accepts validated session-only
edits without any storage call until a successful reload read clears its latch.
`Override(source)` edits effective settings only; source labels are nonsecret
identifiers, not permissions. CLI, environment, runtime and extension callers all
use this same operation.

`apply_overrides(source, values)` recursively merges an object into current
effective values. Missing properties and nested siblings survive; arrays, scalars
and null replace whole values. Supplied leaves get the source's Override origin;
untouched leaves retain theirs. Scope maps and storage remain unchanged. The same
manifest guard applies, including equal restatement and locked subtree membership.
Unlike overlay, `set` still replaces its addressed subtree/root exactly.

`reload()` attempts both scope reads and discards overrides only after successful
validation. Memory-backed initial values and persisted scoped edits survive.
Any failure retains both accepted scope maps and the entire effective snapshot,
including overrides. Read failures queue diagnostics in user/project order and
return the first error. Each scope's persistence latch follows its own read result,
even if the other read fails and nothing publishes. A parse-successful lock conflict
is not a load failure. Re-apply overrides through `set` or `apply_overrides`.

## Errors

- `InvalidLock`: empty manifest lock path or traversal through a nonobject.
- `MissingLockTarget`: target absent from the engine-plus-manifest baseline.
- `InvalidPath`: setter traversal through a nonobject or a nonobject root value.
- `LockConflict`: attempted frozen-value change or omission, with Manifest
  authority and attempted origin.
- `Storage`: adapter failure, with the affected stored scope.
- `File`: scope, selected file path and I/O category, malformed JSON line/column,
  nonobject root, exhausted contention or cancellation. Parser excerpts are not retained.
- `Location`: fixed input name and origin, never the input value. Invalid absolute
  process/home roots use Engine origin. A non-string `sessionDir` reports its
  effective origin; nonempty objects report their highest-precedence descendant
  origin (override > project > user > manifest > engine).

Display and Debug contain paths and source metadata, never attempted/frozen
values or serialized settings. Use nonsecret override source labels. Invalid and
missing lock diagnostics name Manifest authority.

## Storage seam and example

`Settings::new(engine, manifest, Box<dyn SettingsStorage>)` injects storage below
the same resolver. `SettingsStorage::transact` invokes its `SettingsTransaction`
callback exactly once after admission and parsing a current scope map:
`Ok(None)` leaves settings bytes unchanged,
`Ok(Some(next))` commits a replacement, and `Err` rejects without mutation. The
adapter owns only scoped read-modify-write, never precedence or value-lock policy.
Required `read` returns detached values without creation or a mutation lock; missing
is empty. Admission/read failures invoke the transaction callback zero times.
Transactions may create their directory/sidecar even when a callback returns None.

```rust
use maestro_settings::{ManifestSettings, MemorySettingsStorage, Settings,
    SettingsScope, SettingsTarget};
use serde_json::{Map, json};

let mut settings = Settings::new(
    Map::new(),
    ManifestSettings { values: Map::new(), locks: vec![] },
    Box::new(MemorySettingsStorage::new(Map::new(), Map::new())),
).unwrap();
settings.set(SettingsTarget::Stored(SettingsScope::User),
    &["quietStartup".into()], json!(true)).unwrap();
assert_eq!(settings.read_scope(SettingsScope::User)["quietStartup"], json!(true));
settings.apply_overrides("runtime".into(),
    json!({"display":{"width":80}}).as_object().unwrap().clone()).unwrap();
settings.apply_overrides("runtime".into(),
    json!({"display":{"theme":"dark"}}).as_object().unwrap().clone()).unwrap();
assert_eq!(settings.resolve().values["display"], json!({"width":80,"theme":"dark"}));
assert!(settings.drain_errors().is_empty());
assert_eq!(settings.reload().unwrap().values["quietStartup"], json!(true));
```

Crate rustdoc contains executable memory and pure file-construction examples.
Shared integration cases use identical calls with memory, controlled and file
adapters; callers have no adapter-dependent policy.

## Explicit file composition

The composition caller supplies process cwd, home and the selected user root.
No ambient cwd/home/environment lookup happens inside this crate. Both files
contain one current JSON object: selected-root/settings.json and
working-directory/.maestro/settings.json. Unknown keys remain ordinary data.
Construction of the adapter alone does no I/O; Settings construction reads both
scopes. Missing reads/reload create no configuration directories or sidecars.

```rust
use maestro_settings::{FileSettingsStorage, SettingsLocations, Settings,
    ManifestSettings};
use serde_json::Map;
use std::{path::PathBuf, sync::{Arc, atomic::AtomicBool}};

// Synthetic paths: adapter construction and selection do not access the disk.
let locations = SettingsLocations::new(PathBuf::from("/synthetic/process"),
    Some(PathBuf::from("work")), PathBuf::from("config"),
    PathBuf::from("/synthetic/home")).unwrap();
let storage = FileSettingsStorage::new(locations.clone(),
    Arc::new(AtomicBool::new(false)));
// In a caller-owned isolated directory, the same memory composition becomes:
// let settings = Settings::new(Map::new(),
//     ManifestSettings { values: Map::new(), locks: vec![] }, Box::new(storage))?;
```

## Session and resource paths

`session_directory(&locations, explicit, environment)` selects:

1. Explicit caller option (including empty string).
2. Nonempty supplied `MAESTRO_SESSION_DIR` environment value.
3. Effective `sessionDir` setting (including empty string).
4. Selected user-root/sessions.

A selected non-string setting is an error. Explicit/environment winners are
validated as raw overrides by the existing manifest value guard, with `cli` and
`environment` origins, before expansion. Equal raw restatement succeeds; an
absolute path equivalent to a frozen relative value does not bypass its lock.
Shadowed environment input is not another edit. Selection never changes snapshots,
persists preferences, creates directories, opens sessions or activates resources.
The session owner owns subsequent layout and opening.

Invocation, resource and selected session text is trimmed at both ends (including
U+FEFF, excluding U+0085). Empty trimmed input selects its base. `~`, `~/x` and `~x`
expand under supplied home. Absolute input replaces its base; relative invocation
and session paths (even user-scope sessionDir) use the effective working directory.
`resource_path(path, declaring_directory)` uses the supplied user/project,
manifest or package directory; a relative declaring directory resolves at cwd.
All results normalize repeated separators, `.` and `..` lexically, including
absolute/home-expanded input. Surplus `..` cannot escape an absolute root.
`link/../x` selects base/x regardless of whether link exists or is a symlink.
Native path characters are not lossy-converted. No canonicalization, existence
check, discovery or activation occurs. Raw session values stay unchanged;
trimmed, expanded or lexically equivalent text cannot restate a different frozen
raw string. Constructor root-selection policy is unchanged: relative working
and user directories anchor as before, with exact `~` and `~/` home expansion.

## Native exclusion, failures and repair

A scoped transaction creates its parent and persistent settings.json.lock sidecar,
opens an independent non-truncating read/write native handle and uses its exclusive
lock. It re-reads settings only after acquisition, including a missing first file.
The owned handle remains held through callback, complete serialization and in-place
write; closing it releases only its lock. The sidecar is never removed as release.
The synchronous policy is 10 acquisition attempts with nine inter-attempt blocking
20 ms waits on contention: no busy spin, final wait, lease or overall deadline.
Native noncontention errors stop immediately. A caller-owned cancellation signal
starts false; true cancels before admission, including after a wait or acquisition.
The adapter never resets it. An admitted transaction settles despite late cancellation.
File-write locks do not alter manifest value locks.

Malformed, empty and nonobject files remain storage errors; only NotFound means
empty. Settings construction attempts both loads, substitutes empty contributions
for failed scopes and keeps healthy settings usable. `drain_errors()` returns and
removes scoped File/Storage load diagnostics without changing bytes or clearing
persistence latches; a second drain is empty. Manifest and stored-layer governance
failures still reject construction. Synchronous setter/path/lock/transaction
failures return directly, rather than becoming load diagnostics.

After a failed startup or reload read, that scope's valid stored setters return Ok
and update scoped/effective cache only; malformed bytes are untouched. The healthy
other scope can still persist. Repairing bytes alone does not enable writes:
repair to a JSON object, then reload. Each successful scope read clears its latch;
a successful full reload replaces cached values from disk. Subsequent setters
persist normally and do not replay earlier session-only edits. A nonlatched
transaction encountering malformed bytes still returns Err without publication.

Serialization completes before destination truncation. Cooperating writers preserve
unrelated edits/unknown keys by editing only a fresh scoped map. Native I/O failure
during writing can leave a partial file; the effective snapshot publishes only on
success. Reads are lock-free and a racing read may fail while preserving the previous
snapshot. There is **no** fsync/crash durability, recovery, rollback, whole-file atomic
publication, cross-scope atomic snapshot, symlink-alias coordination or exclusion of
external editors ignoring the sidecar. Linux native tests qualify exclusion only;
memory tests do not establish persistent durability or other platform support.

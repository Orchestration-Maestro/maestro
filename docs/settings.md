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
cannot change a locked subtree. Rejection changes neither the accepted snapshot
nor stored settings.

`Stored(User)` and `Stored(Project)` edit the selected transaction's current map,
retaining unrelated keys, then rebuild effective settings without old overrides.
`Override(source)` edits effective settings only; source labels are nonsecret
identifiers, not permissions. CLI, environment, runtime and extension callers all
use this same operation.

`reload()` re-reads both scopes and discards overrides only after successful
validation. Memory-backed initial values and accepted scoped edits survive.
Failure preserves the last usable snapshot. Re-apply overrides through `set`.

## Errors

- `InvalidLock`: empty manifest lock path or traversal through a nonobject.
- `MissingLockTarget`: target absent from the engine-plus-manifest baseline.
- `InvalidPath`: setter traversal through a nonobject or a nonobject root value.
- `LockConflict`: attempted frozen-value change or omission, with Manifest
  authority and attempted origin.
- `Storage`: adapter failure, with the affected stored scope.

Display and Debug contain paths and source metadata, never attempted/frozen
values or serialized settings. Use nonsecret override source labels. Invalid and
missing lock diagnostics name Manifest authority.

## Storage seam and example

`Settings::new(engine, manifest, Box<dyn SettingsStorage>)` injects storage below
the same resolver. `SettingsStorage::transact` invokes its `SettingsTransaction`
callback exactly once on a current scope map: `Ok(None)` reads without writing,
`Ok(Some(next))` commits a replacement, and `Err` rejects without mutation. The
adapter owns only scoped read-modify-write, never precedence or lock policy.

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
assert_eq!(settings.reload().unwrap().values["quietStartup"], json!(true));
```

Crate rustdoc contains an executable example including origin inspection and
lock rejection. Reusable integration cases accept a storage factory; memory and
a controlled transaction adapter run the same operations without caller policy.
This does not qualify a file adapter. File persistence, filesystem locking,
contention, parsing, discovery, trust admission and sandboxing are outside this
module's guarantees.

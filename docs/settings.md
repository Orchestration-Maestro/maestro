# Settings

`maestro-settings` owns the accepted global and project preferences. A
`SettingsManager` loads both scopes through a replaceable raw-text storage,
answers typed reads from memory, publishes every edit immediately and persists
edits through one ordered queue.

| Scope | Holds |
|-------|-------|
| Global | Preferences for every project |
| Project | Preferences for one working directory; they override global values |

The application chooses the directories. `SettingsManager::create` and
`FileSettingsStorage::new` take the working directory, the agent directory and the
configuration directory name, and read or write `settings.json` below the agent
directory (global) and below the configuration directory in the working directory
(project). Paths are `Path` and `OsStr` values, so names that are not valid UTF-8
work unchanged.

```rust
use maestro_settings::{Settings, SettingsManager};

let mut settings = SettingsManager::in_memory(Settings::default());
settings.set_theme("light".into());
assert_eq!(settings.get_theme().as_deref(), Some("light"));
```

## Reading

Settings are plain JSON objects. Unknown keys and values of an unexpected type are
kept exactly as written and are never discarded, because no schema validates the
whole document. Loading changes a key only through the four stored-format conversions
below, which rename or remove legacy keys.

- A typed read returns an owned value. Mutating it never changes the manager.
- A value of the wrong type reads as if the key were unset, using that key's own
  default; the raw document is not changed. A record such as the compaction
  settings falls back member by member.
- `get_global_settings` and `get_project_settings` return owned copies of the raw
  scope documents.
- Typed records such as `WarningSettings` and `ThinkingBudgetsSettings` are views over
  the stored object. A getter reads one member with its own fallback. A setter
  replaces a member where it stands, appends a new member at the end or removes it
  for `None`. Every other member keeps its value and its position, so reading a
  record and writing it back saves the same member order.
- Lists keep every member. Non-string members of a list, and package entries whose
  shape cannot be typed, are returned as `Unknown` values and written back unchanged.
  A package object whose `source` is a string and whose resource lists are arrays is a
  `FilteredPackage`, a view with the same rules.
- Defaults are results of the getters. They are never inserted into a document.

### Merging

Project settings override global ones. The two documents merge at the top level; a
key that holds an object in both scopes merges one level deep. Arrays, scalars and
`null` replace the global value. The merge is not recursive:

```json
// global
{ "theme": "dark", "retry": { "enabled": false, "provider": { "timeoutMs": 11, "maxRetries": 2 } } }
// project
{ "retry": { "provider": { "maxRetryDelayMs": 3 } } }
// effective
{ "theme": "dark", "retry": { "enabled": false, "provider": { "maxRetryDelayMs": 3 } } }
```

`apply_overrides` merges runtime preferences onto the effective settings the same
way. Overrides are never saved. Any setter, and `reload`, rebuilds the effective
settings and discards them.

## Writing

Every setter publishes the new value to memory before it returns, then queues one
save of its scope; a scope whose load failed is never saved (see Failures). The
queue is shared by both scopes and runs saves in order on a background thread (the
browser runs the same saves on its task queue), so no runtime is needed to start a
write. Saves progress without `flush`; `flush` returns a future that completes once
the work queued before the call has run. Dropping that future does not cancel any
write.

A save captures the accepted scope document and the edits made since the last
successful save, each with a revision. When it runs it reads the storage again,
applies the stored-format conversions to that fresh data, copies only the captured
edits over it and writes two-space JSON without a final newline. Other changes made
to the file in the meantime survive and enter the manager only on `reload`. A
successful save acknowledges only the revisions it wrote, so an edit made after the
save was captured stays pending.

Unsetting `shellPath`, `shellCommandPrefix`, `npmCommand` or `enabledModels`
removes the key. A setter on a nested key such as `compaction.enabled` replaces a
parent value of another type with an object that holds the edit. A number setter
stores `NaN` as `null`, and the image width setter also stores positive infinity as
`null`. After `set_editor_padding_x` or `set_autocomplete_max_visible` receives
`NaN`, the typed read returns `NaN` until a finite setter, a project value or the
next `reload` supersedes it. A runtime override supersedes it only while the override
lasts: the next setter discards the overrides and the accepted `NaN` returns.

### Failures

A scope that fails to load or save records a `SettingsError` with its scope and
cause. `drain_errors` returns the recorded errors once, in order. A failed save
keeps its edits pending and does not stop later saves. A scope whose load failed
still accepts edits in memory but is never written until a `reload` loads it
successfully; draining errors does not lift that block. A document whose root is
not an object, or that is not valid JSON, is a load failure and is never
overwritten.

`reload` waits for queued saves, loads global then project, accepts each healthy
scope independently, keeps the accepted data of a failed scope, and discards
runtime overrides and pending edits. Nothing is printed by any of these operations.

## Storage

`SettingsStorage::with_lock` exchanges raw text for one scope. Once the adapter has
acquired its exclusion and read the scope, it calls the update callback exactly once,
synchronously, with the current text (`None` if the scope was never written); an
adapter that fails to acquire or read returns that failure without calling it. The
callback returns `Ok(None)` to leave the stored text untouched, `Ok(Some(text))` to
replace it, or an error, which is returned without writing.

`InMemorySettingsStorage` keeps one text per scope and holds it exclusively while the
callback runs. `FileSettingsStorage` (not available for the browser build) works in
place:

1. For an existing file it locks the stable `settings.json.lock` sidecar before
   reading, keeps the lock through the callback and the write, and releases it on
   every outcome. The sidecar is never removed.
2. For a missing file it runs the callback first without creating anything. Only a
   replacement creates the directory, takes the lock and writes, without reading again.
3. A contended lock is tried ten times with 20 ms between attempts. Any other
   failure is returned at once.

Writes are not atomic: there is no rename, rollback or sync step.

## Stored-format conversions

Exactly four conversions run when a document is seeded, loaded or read fresh for a
save:

| Stored form | Becomes |
|-------------|---------|
| `queueMode` without `steeringMode` | `steeringMode` |
| boolean `websockets` without `transport` | `transport` of `websocket` or `sse` |
| `skills` object | `enableSkillCommands` (if unset) and a `skills` list from `customDirectories`, or no `skills` |
| numeric `retry.maxDelayMs` | `retry.provider.maxRetryDelayMs` when that is unset or `null`; the old key is always removed |

## Preferences

### Model and thinking

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `defaultProvider` | string | unset | Default provider |
| `defaultModel` | string | unset | Default model ID |
| `defaultThinkingLevel` | string | unset | `off`, `minimal`, `low`, `medium`, `high` or `xhigh`; other text is kept |
| `hideThinkingBlock` | boolean | `false` | Hide thinking blocks |
| `thinkingBudgets` | object | unset | Token budgets for `minimal`, `low`, `medium` and `high`; read-only here |

### Display

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `theme` | string | unset | Theme name |
| `quietStartup` | boolean | `false` | Hide the startup header |
| `collapseChangelog` | boolean | `false` | Show a condensed changelog |
| `enableInstallTelemetry` | boolean | `true` | The preference only; this crate sends nothing |
| `doubleEscapeAction` | string | `tree` | `tree`, `fork` or `none` |
| `treeFilterMode` | string | `default` | `default`, `no-tools`, `user-only`, `labeled-only` or `all`; anything else reads as `default` |
| `editorPaddingX` | number | `0` | The setter rounds down into 0 to 3 and keeps `NaN`; a loaded number is returned as stored and another type reads as the default |
| `autocompleteMaxVisible` | number | `5` | The setter rounds down into 3 to 20 and keeps `NaN`; a loaded number is returned as stored and another type reads as the default |
| `showHardwareCursor` | boolean | `false` | Unset, `null` or another type falls back to `MAESTRO_HARDWARE_CURSOR=1` |
| `markdown.codeBlockIndent` | string | two spaces | Indentation of code blocks |
| `warnings.anthropicExtraUsage` | boolean | unset | The getter returns the stored members only; the consumer applies its own default |

### Context

| Setting | Type | Default |
|---------|------|---------|
| `compaction.enabled` | boolean | `true` |
| `compaction.reserveTokens` | number | `16384` |
| `compaction.keepRecentTokens` | number | `20000` |
| `branchSummary.reserveTokens` | number | `16384` |
| `branchSummary.skipPrompt` | boolean | `false` |

### Retry

| Setting | Type | Default |
|---------|------|---------|
| `retry.enabled` | boolean | `true` |
| `retry.maxRetries` | number | `3` |
| `retry.baseDelayMs` | number | `2000` |
| `retry.provider.timeoutMs` | number | unset |
| `retry.provider.maxRetries` | number | unset |
| `retry.provider.maxRetryDelayMs` | number | `60000` |

The crate stores and returns these values; applying them belongs to the callers.

### Delivery and transport

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `steeringMode` | string | `one-at-a-time` | `all` or `one-at-a-time`; empty text reads as the default |
| `followUpMode` | string | `one-at-a-time` | `all` or `one-at-a-time`; empty text reads as the default |
| `transport` | string | `auto` | `sse`, `websocket`, `websocket-cached` or `auto`; any other text, including empty, is kept |

### Terminal and images

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `terminal.showImages` | boolean | `true` | Show inline images |
| `terminal.imageWidthCells` | number | `60` | Rounded down with a minimum of 1; unusable values read as 60; the setter stores `NaN` and positive infinity as `null` |
| `terminal.clearOnShrink` | boolean | `false` | A stored boolean wins and a stored `null` reads `false`; when unset or of another type, `MAESTRO_CLEAR_ON_SHRINK=1` enables it |
| `terminal.showTerminalProgress` | boolean | `false` | Show terminal progress indicators |
| `images.autoResize` | boolean | `true` | Resize images for model compatibility |
| `images.blockImages` | boolean | `false` | Withhold images from model providers |

### Commands, sessions and models

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `shellPath` | string | unset | Custom shell path |
| `shellCommandPrefix` | string | unset | Prefix for every shell command |
| `npmCommand` | string[] | unset | Package-manager argument vector |
| `sessionDir` | string | unset | An exact `~` reads as the home directory; a `~/` prefix is joined onto it with the platform's separators, folding `.` and `..` segments and keeping a trailing separator; every other text, and any text while the home directory is unknown, is returned as written, including the empty string |
| `enabledModels` | string[] | unset | Model patterns for cycling |

### Resources

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `packages` | array | `[]` | Strings, or objects with `source` and optional `extensions`, `skills`, `prompts` and `themes` lists |
| `extensions` | string[] | `[]` | Extension paths |
| `skills` | string[] | `[]` | Skill paths |
| `prompts` | string[] | `[]` | Prompt template paths |
| `themes` | string[] | `[]` | Theme paths |
| `enableSkillCommands` | boolean | `true` | Register skills as commands |

The lists are stored and returned as written. Path resolution, patterns, package
installation, update checks and reporting belong to the owners of those features.
An extension path never implies a package.

## Example

```json
{
  "defaultProvider": "local",
  "defaultModel": "model-1",
  "defaultThinkingLevel": "medium",
  "compaction": { "enabled": true, "reserveTokens": 16384, "keepRecentTokens": 20000 },
  "retry": { "enabled": true, "maxRetries": 3 },
  "enabledModels": ["local/*"],
  "packages": ["npm:example", { "source": "git:example", "skills": ["search"], "extensions": [] }]
}
```

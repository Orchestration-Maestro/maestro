# Settings

Use `settings.json` to set preferences for every project or for one working
directory. This reference lists each setting with its type, default and effect,
and explains what happens when a recorded base layer locks a value. Maestro
retains unknown keys, and a stored preference does not bypass a lock. The
[settings guide](sessions/settings.md) explains the same preferences by topic.

## Contents

1. [Locations](#locations)
2. [Reading](#reading)
3. [Writing](#writing)
4. [Storage](#storage)
5. [Saved text](#saved-text)
6. [Stored-format conversions](#stored-format-conversions)
7. [Preferences](#preferences)
8. [Example](#example)
9. [Workspace preference records](#workspace-preference-records)
10. [Nested preference edits](#nested-preference-edits)

## Locations

`maestro-settings` owns the accepted global and project preferences. A
`SettingsManager` loads both scopes through replaceable raw-text storage, answers
typed reads from memory, publishes accepted edits immediately and persists them
through one ordered queue. Its public items live in `settings_manager` and are
re-exported at the crate root.

| Scope | Holds |
|-------|-------|
| Global | Preferences for every project |
| Project | Preferences for one working directory; they override global values |

The application chooses the directories. `SettingsManager::create` and
`FileSettingsStorage::new` take the working directory, the agent directory and the
configuration directory name. They read and write `settings.json` below the agent
directory (global) and below the configuration directory in the working directory
(project). The three names are authored path strings: `maestro-path` joins them
lexically, and they become a native path only at a filesystem call (existence
check, open, write or lock).

```rust
use maestro_settings::{Settings, SettingsManager};

let mut settings = SettingsManager::in_memory(Settings::default());
settings.set_theme("light".into());
assert_eq!(settings.get_theme().as_deref(), Some("light"));
```

## Contents

1. [Reading](#reading)
2. [Host decisions](#host-decisions)
3. [Writing](#writing)
4. [Storage](#storage)
5. [Saved text](#saved-text)
6. [Stored-format conversions](#stored-format-conversions)
7. [Preferences](#preferences)
8. [Example](#example)
9. [Workspace preference records](#workspace-preference-records)
10. [Nested preference edits](#nested-preference-edits)

## Reading

Settings are plain JSON objects. No schema validates the whole document, so
Maestro keeps unknown keys and values of an unexpected type exactly as written and
never discards them. Loading changes a key only through the four stored-format
conversions below, which rename or remove legacy keys.

- A typed read returns an owned value. Mutating it never changes the manager.
- A value of the wrong type reads as if the key were unset and uses that key's own
  default; the raw document is unchanged. A record such as the compaction settings
  falls back member by member.
- `get_global_settings` and `get_project_settings` return owned copies of the raw
  scope documents.
- Typed records such as `WarningSettings` and `ThinkingBudgetsSettings` are views
  over the stored object. A getter reads one member with its own fallback. A setter
  replaces a member where it stands, appends a new member at the end, or removes it
  for `None`. Every other member keeps its value and position, so reading a record
  and writing it back saves the same member order, except that a save writes
  canonical array-index keys first (see [saved text](#saved-text)).
- Lists keep every member. Non-string list members, and package entries whose shape
  cannot be typed, come back as `Unknown` values and stay as read in memory;
  [saved text](#saved-text) normalizes them. A package object whose `source` is a
  string and whose resource lists are arrays is a `FilteredPackage`, a view with
  the same rules.
- Defaults are results of the getters. Maestro never inserts them into a document.

`SettingsListEntry::{from_value, into_value}` and
`PackageSource::{from_value, into_value}` are public canonical JSON adapters.
Reading and writing through a pair keeps unknown values and object members, in
their in-memory order; persistence still follows [saved text](#saved-text). The
application forwards these existing types to configuration consumers.

### Merging

Project settings override global ones. The two documents merge at the top level,
and a key that holds an object in both scopes merges one level deep. Arrays,
scalars and `null` replace the global value. The merge is not recursive:

```json
// global
{ "theme": "dark", "retry": { "enabled": false, "provider": { "timeoutMs": 11, "maxRetries": 2 } } }
// project
{ "retry": { "provider": { "maxRetryDelayMs": 3 } } }
// effective
{ "theme": "dark", "retry": { "enabled": false, "provider": { "maxRetryDelayMs": 3 } } }
```

`apply_overrides` merges runtime preferences onto the effective settings the same
way. Overrides are never saved. Any accepted setter, and `reload`, rebuilds the
effective settings and discards them.

### Read-only base layer

A recorded base layer supplies defaults below your settings. You can override an
unlocked default in `settings.json`. A locked value always comes from the base
layer, even if you saved a different value in your file. Maestro reports the
stored conflict without rewriting the file. An explicit edit of a locked
preference is refused before any preference, pending write or live configuration
changes.

The refusal names the key, the recorded release and time, and the refresh action.
For example, a release that locks the right-side tab entries reports:

```text
Edit refused: `multiplexer.tabBarRight` is locked by git:example/config#v1 (recorded 2026-10-27T12:00:00Z); starting Maestro refreshes the base layer
```

Use the refresh action the refusal reports. Changing an ordinary preference file
alone does not remove the recorded layer's locks. For workspace settings, only the
global layer and user file apply; a project file or project layer cannot change the
workspace daemon. With no recorded global layer, the normal user-file defaults and
reload behavior are unchanged.

A publisher can supply a `BaseSettingsLayer`: a settings document, dotted locked
paths, a source release and its recorded time. `publish_base_layer(scope, layer)`
replaces that scope's layer, and `None` clears it. `get_base_layer` borrows the
current layer. A project layer takes precedence over a global layer.
`get_global_base_layer` borrows only the cached global layer and does not reread its
persisted record.

Effective settings merge the base, global and project documents in that order,
with the same one-level merge. Locked paths then take the base value; an absent
base leaf uses the getter's engine default. A malformed ancestor is replaced by an
object holding the locked base value, or removed when that value is absent. This
repair reports the locked path once. Raw scope snapshots, pending edits and saved
preferences never include the base. Stored conflicts produce diagnostics through
`drain_errors` and rewrite neither preference file.

`SettingsError::kind` classifies base-lock and registered-decider resolutions as
nonfatal `StoredValueIgnored` diagnostics, distinct from `Failure` errors such as
refused explicit edits. Reload keeps the accepted layer and the ordinary
failed-scope rules.

Setters and runtime overrides refuse a locked path before they change memory,
pending edits or the write queue. Global edits check the global layer; project
edits check the project layer when present, otherwise the global layer. Runtime
overrides check the active layer. A project-only lock never blocks a global edit.
A batch with one locked member is refused as a whole, including an edit that
replaces or removes a locked leaf's parent. A refusal records one diagnostic that
names the path, the source release, the recorded time and how to refresh it.
Setter signatures are unchanged.

`is_key_path_locked(field, keys)` lets a caller check a literal path before its own
effects; it also detects locks on ancestors and descendants, but not siblings.
`BaseSettingsLayer` offers the same query to a caller that holds a particular
layer.

Publication uses the existing write queue; `flush` waits for its persistence and
`drain_errors` reports failures. A fresh manager reads persisted layers without a
publisher. The native adapter stores the global record in
`<agent_dir>/base-settings.json` and project records in
`<agent_dir>/state/base-settings.json`, keyed by the lexically normalized absolute
project root. Project state never enters the project folder. Clearing a record
leaves an empty slot in the state file. Without a persisted or published layer,
Maestro opens or creates no additional file.

`SettingsStorage::read_base_layer` and `write_base_layer` are the persistence seam.
Their defaults keep no record, so existing custom adapters remain valid. The
in-memory adapter keeps both scope records; the native adapter reuses its existing
file transaction and sidecar lock. This owner does not read a manifest, validate
policy or decide which settings may be governed.

## Host decisions

A host that needs to constrain preferences can register one `SettingsDecider` with
`SettingsManager::register_decider`. The publisher owns its constraints; the
settings manager owns registration, admission and persistence. This is a host
interface, not a guest component registration. Runtime model and thinking changes
use it through the application, including their RPC and extension setters. Without
a decider, operations behave as usual; read-only base locks still apply.

### Registration and refusal

1. Implement `SettingsDecider::decide` and register the handle before you apply
   runtime overrides. Native handles use `Arc` and require `Send + Sync`; a
   browser host uses `Rc`. Registration rebuilds the current effective view.
2. Return `Ok(())` to admit a `DecisionRequest`, or `Err(message)` to refuse it.
   Callbacks run synchronously, outside the persistence queue and storage locks.
3. Read a refusal from the result of a fallible operation. Settings setters keep
   their void signatures and put the message in `drain_errors` instead.

A second registration returns `A settings decider is already registered` and keeps
the first publisher. There is no chain of deciders.

| Request | Data offered to the decider |
| --- | --- |
| `SettingsEdit` | Scope and every literal path with its requested value, or absence for removal |
| `RuntimeOverrides` | Runtime settings before the one-level merge |
| `Model` | Provider, model ID and requested thinking before capability clamping |
| `ThinkingLevel` | Requested thinking before capability clamping |
| `Tools` | Requested active names, or one candidate during automatic refresh |
| `PackageInstall`, `PackageRemove` | Authored source and selected configuration scope |

A refused edit changes no accepted scope, runtime override, pending edit or saved
file. A refused runtime model or thinking change also leaves active state, history
and change notifications untouched. Admission adds no rollback: an I/O failure
after admission follows the owner's ordinary failure behavior.

### Stored values

`SettingsDecider::resolve` receives only the mutable effective view and returns
diagnostic messages. The manager calls it after base values, scope merging and
locks, and when it applies an admitted runtime override. The manager restores
locked values again after the callback, so resolution cannot weaken a lock. The
callback may change the view but never receives the raw scopes or the persistence
queue. Its messages use the existing settings error channel. Reads and resolution
never rewrite stored conflicting values.

`resolve_stored_values` resolves a restored settings fragment over the current view
through the same composition pipeline. It reports conflicts without changing raw
preferences, pending writes or saved sessions.

The [decision tests](../crates/maestro-settings/tests/settings_decisions.rs)
exercise a refusing publisher and a stored-value resolver through the public
manager. They also check duplicate registration and stored-value preservation.

### Preflighted application writes

An owner whose operation has effects before its settings save calls
`prepare_operation`. It supplies the request, the scope and all derived settings
paths before the first effect. The settings owner asks the decider and checks base
locks, then returns a `PreparedSettingsEdit` without publishing or queuing
anything. `has_decider` lets an owner keep its ordinary late-validation order when
neither a publisher nor a base layer governs its operation.

Keep the token with that operation and consume it with `apply_prepared_edit` at the
original save position. `split_at` separates admitted edits when an operation has
more than one save position; apply those parts in their original order. Use the
preparing manager, before another explicit operation. Dropping a token saves
nothing. Applying it does not ask the decider again.

Model selection admits the requested thinking before the model capability clamp. A
locked thinking default stays unchanged while the active level can clamp lower. The
model change skips only the derived write to that locked key; its other effects
keep their order. Without that lock, derived thinking-default saves behave as
usual.

## Writing

An admitted setter publishes the new value to memory before it returns, then queues
one save of its scope; a scope whose load failed is never saved (see Failures).
Both scopes share the queue, which runs saves in order on a background thread (the
browser runs the same saves on its task queue), so starting a write needs no
runtime. Saves progress without `flush`; `flush` returns a future that completes
once the work queued before the call has run. Dropping that future does not cancel
any write.

A save captures the accepted scope document and the edits made since the last
successful save, each with a revision. When the save runs, it reads the storage
again, applies the stored-format conversions to that fresh data, copies only the
captured edits over it and writes two-space JSON without a final newline. Other
changes made to the file in the meantime survive and enter the manager only on
`reload`. A successful save acknowledges only the revisions it wrote, so an edit
made after the save was captured stays pending.

`set_global_key(field, key, value)` edits one member of a global object through the
same queue. When the save runs, it preserves the other members read from storage;
it does not replace the object with the manager's earlier snapshot.

Unsetting `shellPath`, `shellCommandPrefix`, `npmCommand` or `enabledModels`
removes the key. A setter on a nested key such as `compaction.enabled` replaces a
parent value of another type with an object that holds the edit. A number setter
stores `NaN` as `null`, and the image width setter also stores positive infinity as
`null`.

After `set_editor_padding_x` or `set_autocomplete_max_visible` receives `NaN`, the
typed read returns `NaN` until the same setter stores a number or a `reload` loads
the global scope successfully; a failed global load keeps it. While a project value
or a runtime override defines the preference, the typed read returns that value
instead. The next setter of another preference discards the overrides, and the
accepted `NaN` returns unless a project value defines the preference.

### Failures

A scope that fails to load or save records a `SettingsError` with its scope and
cause. `drain_errors` returns the recorded errors once, in order. A failed save
keeps its edits pending and does not stop later saves. A scope whose load failed
still accepts edits in memory, but Maestro never writes it until a `reload` loads
it successfully; draining errors does not lift that block. A document whose root is
not an object, or that is not valid JSON, is a load failure, and Maestro never
overwrites it.

`reload` waits for queued saves, loads global then project, accepts each healthy
scope independently, keeps the accepted data of a failed scope, and discards
runtime overrides and pending edits. `reload_from_storage` does the same without
waiting; a caller that shares the manager awaits `flush` first, so it holds no
borrow across the wait. None of these operations prints anything.

## Storage

`SettingsStorage::with_lock` and `with_write_lock` exchange raw text for one scope.
Each operation calls the update callback exactly once, synchronously, with the
current text (`None` if the scope was never written). The exception is an adapter
that fails to acquire its exclusion or to read first: it returns that failure
without calling the callback. The callback returns `Ok(None)` to leave the stored
text untouched, `Ok(Some(text))` to replace it, or an error, which is returned
without writing.

`InMemorySettingsStorage` keeps one text per scope. It hands the callback an owned
copy and holds no lock while the callback runs, so the callback may use the same
storage again, in either scope; when callbacks overlap, the last write wins.

`FileSettingsStorage` (not available for the browser build) joins the supplied agent
directory, and the working directory with the configuration directory, with
`settings.json` through the shared `maestro-path` functions, giving the runtime's
join results, then works in place. Its legacy `with_lock` operation behaves as
follows:

1. For an existing file, it locks `settings.json.lock` beside the canonical target
   before reading, keeps that lock through the callback and the write, and removes
   it on every outcome. It follows a symbolic link before choosing the lock path.
2. For a missing file, it runs the callback first without creating anything. Only a
   replacement creates the directory, takes the lock and writes, without reading
   again.
3. Synchronous acquisition delegates to [the shared file-lock operation](locking.md):
   up to ten attempts, with 20 ms only between contended attempts.

Manager saves use `with_write_lock`. The native adapter creates the parent of the
settings path as needed and no other directory: a save through a dangling symbolic
link whose target directory is missing fails with the native not-found error and
leaves the link in place. It takes the same write lock before reading, including
when the settings file is absent, and holds exclusion through the single callback
and the optional write.

Independent manager edits survive concurrent first writes and existing-file saves
when the holder finishes within the writer's attempts; a writer that runs out
reports the contention and writes nothing. A failure to acquire or read skips the
callback; a callback or write failure releases exclusion. `Ok(None)` and callback
errors leave the settings bytes untouched, but may leave a new parent. Locking
alone never creates a settings file.

Raw callers that merge concurrent first writes must choose write intent too:

```rust,no_run
use maestro_settings::{FileSettingsStorage, SettingsScope, SettingsStorage};

let storage = FileSettingsStorage::new("work", "agent", ".maestro");
storage.with_write_lock(SettingsScope::Global, &mut |current| {
    let mut document: serde_json::Value = serde_json::from_str(current.unwrap_or("{}"))?;
    document["theme"] = "light".into();
    Ok(Some(serde_json::to_string_pretty(&document)?))
})?;
# Ok::<(), maestro_settings::SettingsStorageError>(())
```

The legacy operation keeps its noncreating reads and lazy first replacement; it
does not serialize the preparation of concurrent first replacements. Custom
adapters inherit `with_write_lock` by delegation to their own `with_lock`, and
Maestro promises no stronger exclusion for them. Native decorators forward write
intent to keep the native guarantee. In-memory callbacks still run without a lock
and keep last-writer behavior.

The native adapter publishes each write through
[the shared recoverable byte publication](locking.md#recoverable-byte-publication),
following a symbolic link at the settings path. It writes the text in place,
keeping the file's permissions, ownership and hard links, and the next coordinated
writer completes a write that was cut short instead of rolling it back. A read-only
settings file is refused and left unchanged. In-memory writes are not durable.

## Saved text

A save writes the document with [the canonical pretty writer](json.md), which owns
the layout, key order and number spelling. As a result, an integer beyond 2^53 is
saved as the double it names. A loaded document keeps integers from
`-9223372036854775808` up to `18446744073709551615` digit for digit in memory until
it is saved; an integer beyond that range is read as a double. This is a deliberate
representation limit: keeping integers of any size would change how numbers
compare in every crate that reads JSON.

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
| `theme` | string | unset | Theme name; the getter reports unset and the consumer chooses its dark fallback |
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

| Setting | Type | Default | Effect |
|---------|------|---------|--------|
| `compaction.enabled` | boolean | `true` | Allow automatic compaction of session context |
| `compaction.reserveTokens` | number | `16384` | Tokens reserved when deciding whether context needs compaction |
| `compaction.keepRecentTokens` | number | `20000` | Recent context retained alongside a compaction summary |
| `branchSummary.reserveTokens` | number | `16384` | Tokens reserved when building a branch summary |
| `branchSummary.skipPrompt` | boolean | `false` | Skip the "Summarize branch?" question and continue without a summary |

### Retry

| Setting | Type | Default | Effect |
|---------|------|---------|--------|
| `retry.enabled` | boolean | `true` | Allow session-level retries after a retryable failure |
| `retry.maxRetries` | number | `3` | Maximum session-level retry attempts |
| `retry.baseDelayMs` | number | `2000` | Initial session-level retry delay in milliseconds |
| `retry.provider.timeoutMs` | number | unset | Optional timeout passed to the model provider |
| `retry.provider.maxRetries` | number | unset | Optional retry limit passed to the model provider |
| `retry.provider.maxRetryDelayMs` | number | `60000` | Largest accepted provider retry delay in milliseconds |
| `httpIdleTimeoutMs` | number or string | `300000` | Longest wait, in whole milliseconds, for HTTP response headers or the next body chunk; `0` and `disabled` remove the limit |

The HTTP idle timeout accepts nonnegative finite numbers, numeric strings with
optional surrounding whitespace, and `disabled` in any letter case. Numbers are
floored. An absent setting takes `300000`; an invalid stored value returns
`Invalid httpIdleTimeoutMs setting: <value>`. The message shows strings as their
text and other rejected values as compact JSON. The setter returns the same error
for a negative or non-finite number, and changes neither memory nor the settings
file. Both accessors return `Result` with `InvalidHttpIdleTimeout` as the error.

The shared native HTTP client starts with a 300,000 ms idle limit. Startup, session
replacement, a settings-panel change and reload apply the current setting to
subsequent shared-client requests. Session replacement checks and applies this
setting before it updates the footer session, the automatic-compaction marker or
the working directory. An invalid value leaves those unchanged and does not notify
branch-change subscribers.

The limit covers the wait for headers, including connecting and sending, and
restarts after each body chunk. Private clients, including the cloud conversation
transport and proxy-script fetches, keep their own limits. Browser requests use the
platform client. Embedders apply the setting with
[`configure_http_dispatcher`](application/sdk.md#requests).

The crate stores and returns these values; the callers apply them.

### Delivery and transport

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `steeringMode` | string | `one-at-a-time` | `all` or `one-at-a-time`; empty text reads as the default |
| `followUpMode` | string | `one-at-a-time` | `all` or `one-at-a-time`; empty text reads as the default |
| `transport` | string | `auto` | `sse`, `websocket`, `websocket-cached` or `auto`; any other text, including empty, is kept |
| `holdUncheckedText` | boolean | `false` | Withhold assistant text, thinking and partial tool output from session listeners until they are checked |

`holdUncheckedText` only withholds, and at its default it changes nothing. While it
is `true`, the session tells its listeners (the interactive view, the JSON and RPC
modes and SDK subscribers) nothing of a response's text or thinking until the
message ends, and nothing of a tool's partial content or details until the tool
result is complete. Tool-call and status events still arrive. A snapshot that
carries a tool call keeps its text and thinking blocks in place, emptied. The
message end delivers the message as the extensions left it, and extension hooks see
every event as before.

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
| `sessionDir` | string | unset | A leading `~` reads as [Home directory in paths](#home-directory-in-paths); bytes of the home directory that are not valid UTF-8 read as U+FFFD; any text while the home directory is unknown is returned as written, including the empty string |
| `enabledModels` | string[] | unset | Model patterns for cycling |

### Adapters

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `adapters.<port>` | string | unset | Name of the adapter that serves the port; see [ports](ports.md) |

### Resources

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `packages` | array | `[]` | Strings, or objects with `source` and optional `extensions`, `skills`, `prompts` and `themes` lists |
| `extensions` | string[] | `[]` | Extension paths |
| `skills` | string[] | `[]` | Skill paths |
| `prompts` | string[] | `[]` | Prompt template paths |
| `themes` | string[] | `[]` | Theme paths |
| `enableSkillCommands` | boolean | `true` | Register skills as commands |

Maestro stores and returns the lists as written. Path resolution, patterns, package
installation, update checks and reporting belong to the owners of those features.
An extension path never implies a package.

### Home directory in paths

An exact `~` names the home directory. A `~/` prefix puts the rest of the path
below it. Other spellings, including `~name`, stay as written. On Windows, `~\` also
expands in the daemon's new-terminal directory and in resource, prompt, skill,
package and extension paths. Tool paths, application directory options,
`sessionDir` and completion paths accept only `~/`, not `~\`. Maestro reads the
home directory only for forms that expand.

How the rest of the path combines with the home depends on the caller:

- `sessionDir`, resource, prompt, skill, package, completion and extension paths
  use a normalizing join. `.`, `..` and repeated separators resolve, and an empty
  home leaves a relative result. An exact `~` returns the home as supplied, except
  in extension paths, which normalize it too.
- Tool paths and application directory options append the suffix unchanged on every
  platform. For example, `~/a//b` with home `C:\h` gives `C:\h/a//b`. A trailing
  separator in the home stays too: `/h/` plus `~/x` gives `/h//x`. An empty home
  with `~/x` gives `/x`.
- The daemon's new-terminal directory keeps its native rule. On Windows it splits on
  both separators, drops empty components and joins with `\`. On other platforms it
  appends the rest without doubling the home's final slash. An absolute remainder
  still stays below the home directory.

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

## Workspace preference records

`maestro_daemon::settings::model` provides owned records for pane shells,
restoration, updates, headless dimensions, scrollback, worktree directories,
notifications and optional client behavior. The records decode camelCase JSON
members without modifying the input. Paths keep their authored spelling.

Pane shells default to an empty executable, `auto` startup mode and `follow`
directory policy. Restoration resumes agents by default. Headless dimensions are
120 by 40; if either dimension is zero, Maestro diagnoses the pair and selects both
default dimensions. Scrollback defaults to 10,000,000 bytes. Worktrees default to
`~/.maestro/worktrees`. Optional client behavior and history persistence are off.
Update checks and detection catalog checks default on. The update channel is
`preview` only for Windows preview builds; other builds default to `stable`.

Toast delivery defaults to `off` with a one-second delay. Delays from 0 through
3,600 seconds are accepted. Workspace placement defaults to bottom-right; clipboard
notifications default on at bottom-center.

The sibling `appearance`, `audio` and `tabs` modules hold theme aliases and custom
color strings, per-agent sound overrides and ordered tab entries. Sound paths
resolve against a supplied agent directory. `NotificationShowSound::None` never
selects a file. Tab commands default to a five-second refresh interval and a
two-second execution deadline; these records do not execute commands.

Interface records default to an expanded sidebar 26 columns wide, bounded by 18 and
36 columns. Mouse capture, copy-on-select, focus redraw and pane borders,
scrollbars and gaps are enabled. New tabs prompt for a name; new workspaces do not.
The mouse wheel scrolls three lines unless given a nonzero override. Host cursor
rendering is `auto`. Agent ordering is `spaces` (`workspaces` is an accepted alias),
with `priority` available. Status indicators default to `dots`, with `symbols`
available.

`settings::sidebar` holds ordered agent and workspace token rows. Each layout allows
at most 16 rows with 16 tokens per row, including per-agent overrides. Override keys
must be canonical detection agent names. Custom tokens start with `$` and use 1–32
ASCII letters, digits, underscores or hyphens. A styled token object contains
`token` and the optional `fg`, `bold` and `dim`; explicit false values are kept.
Foreground colors accept `#RGB` or `#RRGGBB` and serialize as lowercase six-digit
hex. Styles belong to each occurrence, not to the token name.

### Workspace settings governed by global locks

Set these keys in the global `settings.json`, not the project file. The defaults
below apply when neither a recorded base layer nor your user file supplies a value.
An unlocked base value is a default you can override; a locked value wins, as
described under [Read-only base layer](#read-only-base-layer).

| Setting | Type | Default | Effect |
|---------|------|---------|--------|
| `multiplexer.tabBarRight` | array of entry objects | `[]` | Ordered status entries at the right edge of the desktop tab row; entry types are `zoom`, `hostname`, `datetime`, `text` and `command` |
| `multiplexer.allowNested` | boolean | `false` | Allow a multiplexer client inside an existing pane |
| `multiplexer.remote.manageSshConfig` | boolean | `true` | Manage private SSH connection reuse and keepalive fallbacks; `false` leaves the user's SSH configuration unchanged |
| `daemon.resumeAgentsOnRestore` | boolean | `true` | Resume supported agent conversations when restoring panes |
| `daemon.detectionCatalogCheck` | boolean | `true` | Check for detection catalog updates |
| `update.versionCheck` | boolean | `true` | Check for Maestro releases |
| `update.channel` | string | `stable`, or `preview` for Windows preview builds | Select `stable` or `preview` releases |

### Global workspace projection and reload

`maestro_daemon::settings::io` reads the supplied manager through
`SettingsManager::effective_global_settings`: the persisted global base layer, then
the accepted user document, then the base locks. A registered settings decider then
resolves that view, and the locks are restored again so no resolved value replaces a
locked one. This operation rereads the global layer on startup and on each existing
reload; a read failure keeps the accepted layer and reports an error. With no layer
and no decider, the result equals the raw global snapshot.

Workspace preferences never use project settings, project base layers or runtime
chat overrides. Stored lock conflicts are nonfatal diagnostics; explicit refused
edits remain failures. `load_config` consumes startup global errors and returns
unrelated project errors separately. A read or parse failure emits a structured
warning and uses `SettingsManager::global_base_settings`: the cached base layer with
an empty user scope, resolved by the registered decider and restored to its locks
like the global view.

`reload_config` waits for queued writes before it separates earlier errors, awaits
the existing manager reload, then applies the valid semantic areas. Only a new
global document failure keeps every area and reports `Failed`. Warnings or rejected
areas report `Partial`; a clean reload reports `Applied`. Earlier errors and new
project errors are returned for their own reporting path.

The independent areas are onboarding, theme, UI (including sidebar, toast and
sound), terminal, restore, server, update, advanced, worktrees, experimental and
remote. A malformed area keeps its last accepted settings document and does not
block valid siblings. Documents rejected by runtime checks, such as inverted sidebar
bounds or zero terminal dimensions, do not replace accepted documents.

Current global locks are restored in the retained document before it is decoded
again: locked fields follow the base, while unlocked fields keep their previous
values. If the area has never decoded successfully, it uses `global_base_settings`,
as at startup. Without a base layer, rejected areas keep their current typed values
and do not reapply UI interaction settings. A diagnostic identifies the invalid
area. Correcting the user value restores ordinary overrides of unlocked fields.
Project-only locks do not change this behavior. Removed members return to defaults.

Required fields reject wrong JSON kinds, including positional arrays in place of
records. Optional null values remain absent. Unknown nested keys produce sorted,
deduplicated paths after their area decodes successfully; unknown global settings
stay silent. Typed projection does not rewrite raw values.

The annotated defaults below are available as `settings::DEFAULTS_TEXT`. They are a
guide, not a separate settings file or a startup write. Optional selections are
commented out so the host or build default still applies. The guide uses JSONC;
`settings.json` itself remains JSON.

<!-- workspace-defaults:start -->
```jsonc
{
  // Workspace preferences belong in the global settings.json, not project settings.
  // Unset optional members stay absent. This guide is not written on startup.
  "multiplexer": {
    // Missing onboarding also shows setup; false suppresses it.
    // "onboarding": true,
    "theme": {
      // "name": "catppuccin",
      "autoSwitch": false
      // "darkName": "catppuccin",
      // "lightName": "catppuccin-latte",
      // "custom": {"panelBg": "reset", "accent": "#f5c2e7"}
    },
    // Sidebar width is adjusted to content within these column bounds.
    "sidebarWidth": 26,
    "sidebarMinWidth": 18,
    "sidebarMaxWidth": 36,
    // Initial collapse applies on launch; compact keeps a narrow status rail.
    "sidebarStartCollapsed": false,
    "sidebarCollapsedMode": "compact",
    "mobileWidthThreshold": 64,
    // Disable capture to let the terminal handle clicks; pane apps keep their mouse input.
    "mouseCapture": true,
    "copyOnSelect": true,
    // Cursor policy: auto, native or drawn.
    "hostCursor": "auto",
    // Empty disables pass-through. Combine ctrl, alt, cmd, meta or hyper with +; no shift.
    "rightClickPassthroughModifier": "",
    "redrawOnFocusGained": true,
    // Missing/null means three lines per notch; explicit counts must be nonzero.
    // "mouseScrollLines": 3,
    "confirmClose": true,
    "promptNewTabName": true,
    "promptNewWorkspaceName": false,
    "paneBorders": true,
    "paneOuterBorders": true,
    "paneScrollbars": true,
    "paneGaps": true,
    "showAgentLabelsOnPaneBorders": false,
    "hideTabBarWhenSingleTab": false,
    // Position: top or bottom. Entry types: zoom, hostname, datetime, text, command.
    "tabBarPosition": "top",
    "tabBarRight": [],
    "tabBarRightSeparator": " ",
    // Empty leaves the outer terminal title alone. Escape braces with {{ and }}.
    // Tokens: hostname, workspace, tab, pane, terminal_title.
    "windowTitle": "{hostname}: {workspace}",
    // Ordering: spaces (also accepts workspaces) or priority; indicators: dots or symbols.
    "agentPanelSort": "spaces",
    "statusIndicators": "dots",
    "sidebar": {
      "agents": {
        "rowGap": 0,
        "rows": [["state_icon", "workspace", "tab"], ["agent"]],
        // Canonical agent IDs can replace rows for that agent only.
        "rowsByAgent": {}
      },
      "spaces": {
        "rowGap": 0,
        "rows": [["state_icon", "workspace"], ["branch", "git_status"]]
      }
      // Rows and tokens per row are limited to 16. Custom metadata tokens use $name.
      // A styled token is an object: {"token":"workspace","fg":"#abc","bold":true}.
      // Foregrounds accept only #RGB or #RRGGBB; omitted style bits inherit context.
    },
    "accent": "cyan",
    "toast": {
      // Delivery: off, workspace, terminal or system. Delay range: 0 through 3600 seconds.
      "delivery": "off",
      "delaySeconds": 1,
      "workspace": {"position": "bottom-right"},
      "clipboard": {"enabled": true, "position": "bottom-center"}
    },
    "sound": {
      "enabled": true,
      // Optional mp3 paths are relative to the supplied agent directory.
      // Kind-specific paths override the shared path.
      // "path": "sounds/notification.mp3",
      // "donePath": "sounds/done.mp3",
      // "requestPath": "sounds/request.mp3",
      // Canonical agent settings: default, on or off. Global false always wins.
      "agents": {}
    },
    "remote": {
      // Use private connection reuse and keepalive fallbacks after the user's SSH config.
      // False leaves the user's SSH configuration unchanged.
      "manageSshConfig": true
    },
    "allowNested": false,
    // Requires a compatible outer terminal.
    "kittyGraphics": false,
    // Input-method anchoring is opt-in; an empty agent list applies to any focused pane.
    "revealHiddenCursorForCjkIme": false,
    "cjkImeAgents": [],
    // Shapes: block, steady_block, underline, steady_underline, bar, steady_bar.
    "cjkImeCursorShape": "steady_block",
    // Best-effort input-source switching while prefix mode is active on supported hosts.
    "switchAsciiInputSourceInPrefix": false
  },
  "daemon": {
    // Empty selects the supplied shell, then the platform fallback.
    "paneShell": "",
    // Modes: auto, login or non_login. Auto uses login shells on macOS.
    "paneShellMode": "auto",
    // Directory policy: follow, home, current or an authored path such as ~/Projects.
    "newPaneCwd": "follow",
    "resumeAgentsOnRestore": true,
    // Unattached terminal size; either zero falls back to the complete default pair.
    "headlessCols": 120,
    "headlessRows": 40,
    "worktreesDirectory": "~/.maestro/worktrees",
    "scrollbackLimitBytes": 10000000,
    "paneHistory": false,
    "detectionCatalogCheck": true
  },
  "update": {
    // Channel defaults to stable, except Windows preview builds use preview.
    // "channel": "stable",
    "versionCheck": true
  }
}
```
<!-- workspace-defaults:end -->

## Nested preference edits

`SettingsManager::set_key_path(scope, field, keys, value)` edits a literal
top-level field and nested keys in `SettingsScope::Global` or
`SettingsScope::Project`. `set_global_key_path` is the global-only shorthand. Pass
`Some(value)` to set the leaf, including JSON null, or `None` to remove it. An empty
key slice addresses the field itself. New values publish immediately; `flush` waits
for queued writes and `drain_errors` reports failures. Each write merges the edited
leaf into fresh storage under the existing lock, so independent writers can change
sibling preferences without losing updates. Whole-field and object-key setters
still replace the value they address.

Use `set_global_key_paths` when several leaves form one change. The batch publishes
once and reaches storage in one write, so readers never observe half a change.

Scoped edits use the same locked-path refusal and failed-load protection as typed
setters. A refused edit leaves memory and storage unchanged and records a scoped
error. A failed save keeps the accepted value visible and its edit pending for a
later save; take the failure with `drain_errors`.

`flush` returns a future that does not borrow the manager. When the manager is
shared through a `RefCell`, release the borrow before you await it:

```rust
# use maestro_settings::{Settings, SettingsManager, SettingsScope};
# use serde_json::json;
# async fn example() {
let settings = std::cell::RefCell::new(SettingsManager::in_memory(Settings::default()));
settings.borrow_mut().set_key_path(
    SettingsScope::Project, "custom", &["enabled"], Some(json!(true)),
);
let pending = settings.borrow().flush();
pending.await;
let errors = settings.borrow_mut().drain_errors();
# }
```

## Workspace keybinding reset

Workspace shortcut overrides live in `keybindings.json`, not `settings.json`. The
application keybinding owner provides a timestamped backup and an atomic reset of
only the `multiplexer.*` actions. See
[keybinding reset](keybindings.md#reset-multiplexer-overrides) for backup naming,
preserved entries and failure handling. Saved settings and keybinding files stay
strict JSON; annotated defaults are documentation, not a replacement file format.

## Workspace defaults output

The command-line `workspace_commands::settings::run_defaults` presenter writes the
workspace settings owner's `DEFAULTS_TEXT` unchanged. It reads no settings file and
starts no daemon. The command grammar is `maestro settings defaults`, with no extra
arguments; invalid usage returns 2 before it writes the defaults.

The output is annotated JSONC for reading. Do not replace `settings.json` with it:
saved settings remain strict JSON. Copy the values you select, without the
comments, into your settings document. Executable dispatch is separate from this
presenter; see [workspace command discovery](workspace-commands.md).

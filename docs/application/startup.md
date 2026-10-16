# Application keybindings

`maestro_app::KeybindingsManager` extends the [terminal manager](../terminal/keybindings.md)
with application defaults. `AppKeybinding` remains an open action identifier.
The module `presentation_data::keybindings` exports the ordered `KEYBINDINGS`
table, the borrowed toolkit types and `migrate_keybindings_config`.

`new(user_bindings, config_path)` is inert: direct typed overrides are neither
renamed nor loaded from disk. `Default` supplies application defaults without a
path. Clones share one toolkit manager; inherited matching, replacement and
conflict operations remain available. `get_effective_config` returns an owned
resolved snapshot of known actions.

On native targets, `create(Some(root))` reads `keybindings.json` beneath that
root, including an explicitly empty root. `create(None)` obtains the root from
[application configuration](configuration.md#identity-and-roots). File loading
converts legacy action names in memory without writing the file. Current-name
presence wins over its legacy alias, even if its value is not a binding.
`migrate_keybindings_config` preserves arbitrary raw values and orders known
actions before UTF-16-sorted extras, with canonical numeric keys first.

The loader accepts only a JSON object. Values must be strings or all-string
lists; an empty list disables an action. Unsupported identifiers and empty
strings remain literal values. Other values are ignored. Bytes decode with
lossy UTF-8 before JSON parsing; a BOM is not stripped. Read or parse failures
supply empty overrides. Configuration-root lookup errors propagate unchanged.

`reload` replaces overrides using the original native path, restoring defaults
when the file is missing or unreadable. An absent or empty path leaves runtime
overrides intact. Cloned toolkit handles observe reloads. Loading never installs
the active global manager; callers can install a shared handle using
`maestro_tui::set_keybindings((*manager).clone())`.

File factories, reload and stored paths are native-only. Pure defaults, direct
configuration and raw name conversion are also available on browser targets.

## Native legacy conversions

`configuration::migrations::migrate_auth_to_auth_json()` reads the call-time
[configuration root](configuration.md#identity-and-roots). Existing `auth.json`
skips conversion. Otherwise it combines legacy `oauth.json` records with string
`settings.json` API keys, preferring OAuth providers and retaining open record
fields. It returns converted provider names in source order. Each unreadable
legacy source is skipped independently.

Credential output is published before either source is retired. Publication
errors propagate and a failed new output target is removed for retry, retaining
its authored links. Successful publication writes through links, creating new
Unix targets with mode `0600` subject to umask. OAuth retirement renames its entry
to `oauth.json.migrated`; settings retirement writes in place, preserving existing
links and target permissions. Retirement failures are tolerated. Empty admitted
sources can retire without creating auth output. This is not a crash transaction
or concurrent-writer protocol.

`migrate_sessions_from_agent_root()` scans direct `.jsonl` children. An admitted
first-line session header with a nonempty string cwd selects the existing
[session directory encoding](../sessions/session_manager.md). The original entry
is relocated using its native basename; transcript bytes are not rewritten.
The scan captures headers and final link referents before moving any entry.
Relative links are re-anchored because relocation changes their resolution base.
On Windows without link privilege, a link whose target cannot be shown to resolve
to the same file from its new place stays where it is and is retried.
Other creation failures also retain the source.
Absolute links are renamed first. After relative links are published, their final
identities are checked against the captured referent's new location after a successful
move, or its original location otherwise. An absolute link retains its original entry
only when that identity still matches; mere readability is insufficient. Changed
referents are re-anchored through the same link publisher. File operation failures
remain tolerated as described below.
Non-link entries are renamed unchanged.
Existing readable destinations are preserved; dangling destination links are
replaced, not followed. Re-anchored links are staged beside their source with a
`.jsonl.migrating` suffix before publication by rename. An existing staging entry
is never overwritten. Scan and individual-file failures are ignored.
Configuration-root lookup errors propagate from both operations.
These operations are native-only.

### Startup sequence

`run_migrations(cwd, output)` calls credential conversion, root session relocation,
managed binary relocation, persisted key-name conversion, then global and project
command-directory conversion. Finally it collects global hooks/tools notices followed
by project hooks/tools notices. It returns `MigrationResult` with converted provider
names and ordered notices; it neither displays those notices nor installs a manager.
The supplied cwd selects project configuration, independently of the process cwd.

Managed binaries `fd`, `rg`, `fd.exe`, `rg.exe` move from `tools/` to `bin/` through
the session relocation owner. Links capture final referents before moves, including
referents below moved directories. Readable destination collisions remove only old
files or links (including Windows directory links), without following links;
two spellings of the same entry are not unlinked. Distinct
hardlinked entries still receive cleanup. Dangling sources are skipped and dangling
destinations are replaceable. Binary-directory creation errors propagate; individual
rename/delete failures are tolerated. The binary staging name appends `.migrating`
to the complete basename and never overwrites an existing staging entry.

Persisted `keybindings.json` conversion uses the same raw name converter described
above, writing only renamed object documents, with two-space JSON and one final LF.
It writes through the authored path, preserving live links and permissions. Read,
parse and write failures are tolerated; malformed, non-object and current-only files
are not rewritten. This differs from ordinary binding loading, which never writes.

Global then project `commands` entries rename to `prompts` unless a readable
destination exists. Successful renames emit success lines; failed renames emit warnings
and continue. Live hooks entries generate notices. Tools scans ignore dot-prefixed
names and case-insensitive exact binary names; other entries generate one notice per
scope. Read failures are tolerated and duplicate notices remain ordered.

### Presenting notices

`show_deprecation_warnings(warnings, output, wait_for_input)` writes ordered warnings,
current documentation guidance and the pause prompt before polling the host's one-shot
future. Empty warnings do nothing, including no input acquisition. A successful wait
writes the final blank line; output and wait errors propagate unchanged without a
success blank line.

`MigrationOutput` receives one logical line without its appended LF. The host selects
the stream, resolves success/warning roles from its brand pack, preserves dim decoration
and appends exactly one LF, including for the final plain empty line. Embedded newlines
and decoration remain in the payload. No global output stream or palette is installed.
Unix hosts may supply [`terminal::wait_for_input`](../terminal/native-terminal.md#one-input-wait).
Frontend startup invocation and manager installation remain separate operations.
See [ordinary command metadata](commands.md) for the presentation inventory.

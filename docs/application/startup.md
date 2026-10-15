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
The scan captures headers and resolved link referents before moving any entry.
Links are re-anchored to their referent's new location after a successful move,
or its original location if it did not move. Other entries are renamed unchanged.
Existing readable destinations are preserved; dangling destination links are
replaced, not followed. Re-anchored links are staged beside their source with a
`.jsonl.migrating` suffix before publication by rename. An existing staging entry
is never overwritten. Scan and individual-file failures are ignored.
Configuration-root lookup errors propagate from both operations.
These operations are native-only and do not install startup orchestration.

# Terminal keybindings

`maestro_tui::keybindings` resolves open action IDs against key identifiers.
The crate root re-exports its types, `TUI_KEYBINDINGS`, `get_keybindings` and
`set_keybindings`. Matching delegates to the [key decoder](keys.md), reads the
current protocol mode and does not filter release events.

`KeybindingsManager::new` uses only the supplied definitions. Pass
`TUI_KEYBINDINGS.clone()` for the 31 built-in editor, input and selector actions;
append a definition to include a custom action. Definitions carry default keys
and optional descriptions. `KeybindingKeys::Single` and `Multiple` retain the
supplied scalar/list shape. Definitions and configurations are ordered pairs:
repeated action IDs replace the value without moving its first slot. All IDs,
including numeric-looking and prototype-like names, are ordinary open text.

An absent override or `None` uses defaults. `Some(Multiple(vec![]))` disables
an action. Other overrides replace its defaults, never append. Normalization
removes duplicate literal keys while preserving first occurrence order; it does
not trim or change casing. Unsupported identifiers remain stored but do not match.

`get_definition` returns the original definition shape or `None` for an unknown
action. `get_user_bindings` retains list repetitions, unset values and unknown
actions. `get_keys` returns normalized keys or an empty list. `get_resolved_bindings`
includes only defined actions: one key becomes `Single`, zero or multiple keys
become `Multiple`, and every value is present.

Only explicit overrides for multiple distinct known actions create conflicts.
Defaults and unset overrides never claim keys. `get_conflicts` compares literal,
case-sensitive identifiers, retaining first-key-claim and first-claimant order.
Sharing a key never removes bindings from another action.

`set_user_bindings` replaces the entire configuration and publishes rebuilt keys
and conflicts together. Every observation returns an owned snapshot, including
nested lists, so editing it cannot change the manager. Cloning the manager itself
shares identity: later replacements are visible through all its handles.

`get_keybindings` lazily creates the active manager with built-in defaults.
`set_keybindings` replaces the active manager for future lookups without changing
retained old handles. Tests can restore a saved handle through the same setter;
there is no separate reset API. These operations do not implement widgets,
configuration-file loading or application-specific actions.

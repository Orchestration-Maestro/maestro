# Owned extension registration

`maestro-extensions` registers commands, flags and shortcuts on caller-supplied
extension owners. It does not load extension artifacts or create application contexts.
The caller supplies provenance, an existing event bus, effective keybindings and
its current UI-presence observation. The loader or runtime retains a strong extension
owner. API clones hold weak owner handles, so callbacks may capture them without
keeping a discarded extension alive. After the last strong owner is released,
registration and facade flag reads return the stale-context error.

## Register and query

```rust
use maestro_extensions::{create_event_bus, Extension, ExtensionAPI, ExtensionRunner, SourceInfo};
use maestro_extensions::types::{
    ExtensionRuntimeState, RegisteredCommandOptions, ExtensionFlagOptions,
    ExtensionShortcutOptions, FlagType, FlagValue,
};
use std::sync::{Arc, RwLock};

# fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
let source: SourceInfo = serde_json::from_value(serde_json::json!({
    "path": "/extensions/example", "source": "cli",
    "scope": "temporary", "origin": "top-level"
}))?;
let owner = Arc::new(RwLock::new(Extension::<(), ()>::new(
    source.path.clone(), "/resolved/example".into(), Arc::new(RwLock::new(source)),
)));
let owners = Arc::new(RwLock::new(vec![Arc::clone(&owner)]));
let runtime = Arc::new(ExtensionRuntimeState::new());
let bus = create_event_bus(
    Arc::new(|tail| { tokio::spawn(tail); }),
    Arc::new(|line| eprint!("{line}")),
);
let api = ExtensionAPI::new(&owner, Arc::clone(&runtime), bus.for_extension());
api.register_command("hello".into(), RegisteredCommandOptions {
    description: Some("Say hello".into()),
    get_argument_completions: None,
    handler: Arc::new(|args, ()| Box::pin(async move {
        assert_eq!(args, "raw arguments");
        Ok(())
    })),
})?;
api.register_flag("quiet".into(), ExtensionFlagOptions {
    description: None, r#type: FlagType::Boolean,
    default: Some(FlagValue::Boolean(false)),
})?;
api.register_shortcut("ctrl+x".into(), ExtensionShortcutOptions {
    description: None, handler: Arc::new(|()| Box::pin(async { Ok(()) })),
})?;
let runner = ExtensionRunner::new(owners, runtime, Arc::new(|line| eprint!("{line}")));
assert_eq!(api.get_flag("quiet")?, Some(FlagValue::Boolean(false)));
assert!(runner.get_shortcuts(&vec![], false).contains_key("ctrl+x"));
let command = runner.get_command("hello").ok_or("command missing")?;
let executor = tokio::runtime::Builder::new_current_thread().build()?;
executor.block_on((command.handler)("raw arguments".into(), ()))?;
# Ok(())
# }
```

## Commands

Same-owner replacement keeps the original insertion position. Queries read the
current load and registration order. Duplicate base names receive one-based
suffixes, such as `hello:1` and `hello:2`. If an invocation name is already taken,
its suffix advances until free; future literal names are not pre-reserved.
Lookup uses the resolved invocation name, not a duplicated unsuffixed base name.
Listing commands publishes a fresh empty command-diagnostic collection.

Argument completion is optional. A supplied callback returns `None`, an empty
candidate list or ordered candidates with value, label and optional description.
Arguments and completion prefixes are passed unchanged; callback failures remain
catchable errors. Resolved commands copy their top-level fields but retain the
registered source object and callbacks.

## Flags and activity

Flag metadata keeps the first owner across extensions. Defaults seed only missing
shared values, without validating the default against the declared kind. False
and empty strings remain present. An owner can read only names it registered;
the runner can set any name. Value snapshots are independent ordered maps, while
returned flag records retain their owners.

Invalidation retains the first message, including an empty string. Registration
and facade flag reads then return that error before effects. Runner queries and
its flag setter remain available. Omitting the message uses guidance about stale
contexts after session replacement or reload.

## Shortcuts and diagnostics

Matching lowercases supplied keys without changing raw registration text or the
captured owner path. Effective bindings for editor-global reserved actions block
overrides. Other built-in collisions warn but allow overrides. Later extension
registrations replace normalized-key collisions in the first key's slot.

Each shortcut query publishes fresh diagnostics; an earlier retained collection
is not cleared. The `has_ui` input suppresses only warning output, not diagnostics
or selection. A warning reporter receives one complete line, including its final
newline, outside registration locks. Flag and shortcut paths are captured when
registered; changing an owner's current path does not rewrite them.

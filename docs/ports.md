# Ports and adapters

Maestro reaches each outside technology through a port. A port names the
operations Maestro needs as a trait; an adapter implements it. One registry
holds every adapter, and one settings key per port picks which one runs. Each
port names a default adapter, so an unset key still selects one. Swapping a
technology replaces an adapter, never a caller.

## Quick start

1. Declare the port next to the code that calls it. The trait is the contract;
   the second argument names the default adapter:

   ```rust
   use maestro_app::adapters::Port;

   trait Store: Send + Sync {}

   const STORE: Port<dyn Store> = Port::new("store", "embedded");
   assert_eq!(STORE.id(), "store");
   ```

2. Register an adapter under a name:

   ```rust
   use std::sync::Arc;
   use maestro_app::adapters::{Port, Registry};

   trait Store: Send + Sync {}
   struct Embedded;
   impl Store for Embedded {}
   const STORE: Port<dyn Store> = Port::new("store", "embedded");

   let mut registry = Registry::default();
   registry.register(&STORE, "embedded", || {
       Arc::new(Embedded) as Arc<dyn Store>
   })?;
   # Ok::<(), maestro_app::adapters::AdapterError>(())
   ```

3. Pick another adapter in `settings.json`. Leave the key out to run the default:

   ```json
   { "adapters": { "store": "server" } }
   ```

4. Build the chosen adapter with `Registry::select(&STORE, &settings)`.

## Registering

`Registry::register(port, name, factory)` admits one adapter. Maestro refuses the
registration, and changes nothing, when the port already has an adapter with that
name (`AdapterError::Duplicate`).

An adapter that lacks an operation never reaches the registry. The factory must
return `Arc<dyn Trait>`, and a type without every method of the trait does not
compile:

```rust,compile_fail,E0277
use std::sync::Arc;
use maestro_app::adapters::{Port, Registry};

trait Store: Send + Sync {
    fn get(&self, key: &str) -> Option<String>;
}
struct Lacking;
const STORE: Port<dyn Store> = Port::new("store", "lacking");

let mut registry = Registry::default();
registry.register(&STORE, "lacking", || Arc::new(Lacking) as Arc<dyn Store>)?;
# Ok::<(), maestro_app::adapters::AdapterError>(())
```

A later registration adds a vendor. No caller changes: callers hold the port,
never an adapter.

## Selecting

`Registry::select(port, settings)` reads `adapters.<port>` from the effective
settings and calls the registered factory for that name. Each call builds a new
adapter.

| Situation | Result |
| --- | --- |
| The key names a registered adapter | The adapter |
| The key is unset | The port's default adapter |
| The key is not a string, or `adapters` is not an object | `AdapterError::Setting`, naming the key and the value |
| The key names an unregistered adapter | `AdapterError::Unknown`, listing the installed names |

A refusal reads, for example:

```text
no adapter `third` for port `store`; installed: embedded, server
```

`Registry::installed()` lists every registration, in the order it was admitted,
as a port and a name. Use it for status reports.

## Settings

The setting is the `adapters` object, with one string key per port.

| Setting | Type | Default | Description |
| --- | --- | --- | --- |
| `adapters.<port>` | string | The port's default adapter | Name of the adapter that serves the port |

`SettingsManager::get_adapter(port)` reads the effective name. A project value
overrides a global one. A value that is not a string is refused, never
replaced by the default. Write the name with `set_key_path(scope, "adapters", &[port], Some(name))`.

A base layer locks an adapter key like any other. Lock `adapters.<port>` to fix
one port, or `adapters` to fix them all; a locked key keeps the base name and
refuses edits. See [settings](settings.md#read-only-base-layer).

## Ports

| Port | Settings key | Default adapter | Owner |
| --- | --- | --- | --- |
| None declared yet | | | |

Each port's owner adds its row when it declares the port.

# Ports and adapters

Maestro reaches each outside technology through a port. A port names the
operations Maestro needs; an adapter supplies them. One registry holds every
adapter, and one settings key per port picks which one runs. Swapping a
technology replaces an adapter, never a caller.

## Quick start

1. Declare the port next to the code that calls it:

   ```rust
   use maestro_app::adapters::Port;

   trait Store: Send + Sync {}

   const STORE: Port<dyn Store> = Port::new("store", &["get", "put"]);
   assert_eq!(STORE.id(), "store");
   ```

2. Register an adapter under a name. List the operations it offers:

   ```rust
   use std::sync::Arc;
   use maestro_app::adapters::{Port, Registry};

   trait Store: Send + Sync {}
   struct Embedded;
   impl Store for Embedded {}
   const STORE: Port<dyn Store> = Port::new("store", &["get", "put"]);

   let mut registry = Registry::default();
   registry.register(&STORE, "embedded", &["get", "put"], || {
       Arc::new(Embedded) as Arc<dyn Store>
   })?;
   # Ok::<(), maestro_app::adapters::AdapterError>(())
   ```

3. Select it in `settings.json`:

   ```json
   { "adapters": { "store": "embedded" } }
   ```

4. Build the chosen adapter with `Registry::select(&STORE, &settings)`.

## Registering

`Registry::register(port, name, operations, factory)` admits one adapter. Maestro
refuses the registration, and changes nothing, when:

| Cause | Error |
| --- | --- |
| The port already has an adapter with that name | `AdapterError::Duplicate` |
| `operations` leaves out an operation the port requires | `AdapterError::MissingOperations`, listing the missing names |

A later registration adds a vendor. No caller changes: callers hold the port,
never an adapter.

## Selecting

`Registry::select(port, settings)` reads `adapters.<port>` from the effective
settings and calls the registered factory for that name. Each call builds a new
adapter.

| Situation | Result |
| --- | --- |
| The key names a registered adapter | The adapter |
| The key is unset or not a string | `AdapterError::NotSelected`, listing the installed names |
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
| `adapters.<port>` | string | unset | Name of the adapter that serves the port |

`SettingsManager::get_adapter(port)` reads the effective name. A project value
overrides a global one, and a value of another type reads as unset. Write the
name with `set_key_path(scope, "adapters", &[port], Some(name))`.

A base layer locks an adapter key like any other. Lock `adapters.<port>` to fix
one port, or `adapters` to fix them all; a locked key keeps the base name and
refuses edits. See [settings](settings.md#read-only-base-layer).

## Ports

| Port | Settings key | Required operations | Owner |
| --- | --- | --- | --- |
| None declared yet | | | |

Each port's owner adds its row when it declares the port.

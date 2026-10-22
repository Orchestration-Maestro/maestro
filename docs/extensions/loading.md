# Loading extensions from paths

The extension domain loads extensions from file paths and discovers them in the
project, user and configured locations. Path loading is native only; browser
builds keep `load_extension_from_factory` and have no file loading.

## Module loader

Turning a resolved path into a factory is the only step that depends on the
module format, so the caller supplies it as a `ModuleLoader<E>`. It returns
`Ok(Some(factory))` for a module that exports one, `Ok(None)` for a module that
exports none, and `Err` when the module cannot be loaded. The component runtime
supplies the real loader and the executable wires it in. Like
`ExtensionFactory<E>`, the loader is a shared pointer whose callable carries the
execution family's bounds, so the default family keeps it `Send + Sync`.

## Path loading

`load_extensions(paths, cwd, &event_bus, &module_loader)` creates one runtime
and loads each path in order, awaiting each factory before the next path. All
loaded extensions share that runtime.

Each path is resolved before its module loads:

1. U+00A0, U+2000 to U+200A, U+202F, U+205F and U+3000 become a plain space.
   Other characters, such as tab, U+200B or U+FEFF, are kept.
2. A leading `~` or `~/` is replaced by the home directory, and the rest is
   joined to it lexically. A `~` anywhere else is literal.
3. An absolute result is kept as written. A relative result is resolved
   lexically against `cwd`.

A loaded extension keeps the supplied spelling as its path, and the resolved
path as its resolved path. Its source is `local` and its base directory is the
parent of the resolved path.

The result records a failed path with its supplied spelling, and loading
continues with the next path. The error is one of two messages:

- `Extension does not export a valid factory function: <path>` when the loader
  finds no factory. The path is the supplied spelling.
- `Failed to load extension: <message>` when the loader or the factory fails.

An empty list loads nothing. When the home or working directory is needed and
cannot be read, the whole call fails before any module loads.

## Event bus

The caller supplies the event bus, and each loaded path gets its own
`for_extension()` identity on it. A Maestro bus needs a caller-supplied spawner
and reporter, so the layer that owns the executor builds the default bus. A
caller that wants a fresh bus passes `create_event_bus_for(spawn, report)`.
Two calls with two fresh buses share nothing.

## Discovery

`discover_and_load_extensions(configured_paths, cwd, agent_dir, &event_bus,
&module_loader)` collects paths in this order, then loads them:

1. `<cwd>/.maestro/extensions`
2. `<agent_dir>/extensions`
3. Each configured path, resolved as above. A directory contributes its declared
   entries, or else a scan of its own files. Anything else is added as resolved,
   even when it does not exist, and then fails in the loader.

Discovery compares paths by their lexically resolved form. The first spelling
wins and discovery drops later duplicates. Discovered paths go through the same
resolution when they load, so a discovered name containing one of those spaces
reaches the loader with a plain space.

A scan reads one directory level in name order, hidden names included.
Files and links whose names end in `.ts`, `.js` or `.wasm` are added. Matching
is case-sensitive, so `.d.ts` is admitted and `.TS` is not. Other directories
and links add their declared entries. A missing or unreadable directory adds
nothing, and so does a directory where any child's manifest lists a non-string
item. The same manifest error in a configured directory fails the whole call.

The caller passes the agent directory; the application owns its default.

## Entry rules

The entry rules belong to `maestro_resources::package_manifest`, which package
discovery shares. A directory's entries are the existing paths that the
`maestro.extensions` list in its `package.json` names, in list order. Without
such a list, or when none of its paths exist, the entry is the first of
`index.ts`, `index.js` and `index.wasm` that exists.

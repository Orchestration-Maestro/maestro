# Loading extensions from paths

The extension domain loads extensions from file paths and discovers them in the
project, user and configured locations. Path loading is native only; browser
builds keep `load_extension_from_factory` and have no file loading.

## Module loader

Turning a resolved path into a factory is the only step that depends on the
module format, so the caller supplies it as a `ModuleLoader<E>`. It returns
`Ok(Some(factory))` for a module that exports one, `Ok(None)` for a module that
exports none, and `Err` when the module cannot be loaded. For built components,
a loader that returns `artifact_factory(path, widgets, installation)` from
`maestro-extensions-wasmtime` loads each component through the path, discovery and registration rules below
(see [native component execution](native-execution.md)). The executable supplies
its host widgets and `Installation { version, agent_dir }` from application
configuration. Like
`ExtensionFactory<E>`, the loader is a shared pointer whose callable carries the
execution family's bounds, so the default family keeps it `Send + Sync`.

## Path loading

`load_extensions(paths, cwd, &event_bus, &module_loader)` creates one runtime
and loads each path in order, awaiting each factory before the next path. All
loaded extensions share that runtime.

Each path is resolved before its module loads:

1. A leading `~` reads as
   [settings](../settings.md#home-directory-in-paths) describes, so `~name` stays a
   relative path. A `~` anywhere else is literal.
2. An absolute result is kept as written and reads no working directory. A
   relative result is resolved lexically against `cwd`.
3. The path is resolved through
   [`resolve_existing_spelling`](../../crates/maestro-tools/src/path_utils.rs).
   A broken link counts as missing, because the check follows links.

So a file whose real name holds a no-break space loads under that name, and a
typed name whose Unicode spaces stand for plain spaces still finds its file.

The module loader receives the selected path normalized lexically to an absolute
path: `/a/../b` becomes `/b`, and `/a/./b` becomes `/a/b`. This does not
canonicalize the filesystem path, so directory-symlink spelling is preserved.

A loaded extension keeps the supplied spelling as its path, and the resolved
path as its resolved path, without this final normalization. Its source is
`local` and its base directory is the parent of the resolved path.

The result records a failed path with its supplied spelling, and loading
continues with the next path. The error is one of two messages:

- `Extension does not export a valid factory function: <path>` when the loader
  finds no factory. The path is the supplied spelling.
- `Failed to load extension: <message>` when the loader or the factory fails.

An empty list loads nothing. When the home or working directory is needed for a
path and cannot be read, the whole call fails before that path's module loads.
Earlier paths have already loaded by then.

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
   entries, or else a one-level scan. Anything else is added as resolved, even
   when it does not exist, and then fails in the loader.

Discovery compares paths by their lexically resolved form. The first spelling
wins and discovery drops later duplicates. A found path that names an existing
file loads under its real spelling; a broken link is treated as missing and
resolved again through
[`resolve_existing_spelling`](../../crates/maestro-tools/src/path_utils.rs).

A scan reads one directory level in name order, hidden names included. It adds
the extension files and links, and the declared entries of other directories and
links, following the [entry rules](../packages.md#conventional-resource-discovery).
A missing or unreadable directory adds nothing, and so does a directory where
any child's manifest lists a non-string item. The same manifest error in a
configured directory fails the whole call.

The caller passes the agent directory; the application owns its default.

## Entry rules

The entry rules belong to `maestro_resources::package_manifest`, which package
discovery shares; [package resources](../packages.md#conventional-resource-discovery)
states them.

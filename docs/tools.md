# Native tool factories

`maestro_tools` exposes the native factories at the crate root and in
[`maestro_tools::tools`](../crates/maestro-tools/src/tools.rs). They are unavailable
on `wasm32` targets.

```rust
use maestro_tools::{ToolsManager, ToolsOptions, create_coding_tools, create_tool};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cwd = std::env::current_dir()?.to_string_lossy().into_owned();
    let bin_dir = cwd.clone();
    let search = Arc::new(ToolsManager::new(bin_dir.clone(), None));
    let coding = create_coding_tools(
        cwd.clone(), ToolsOptions::default(), bin_dir.clone(), Arc::clone(&search),
    );
    assert_eq!(coding.iter().map(|tool| tool.definition.name.as_str()).collect::<Vec<_>>(),
               ["read", "bash", "edit", "write"]);
    let read = create_tool("read", cwd, ToolsOptions::default(), bin_dir, search)?;
    assert_eq!(read.definition.name, "read");
    Ok(())
}
```

## Selection and order

`ToolName` parses and displays exactly `read`, `bash`, `edit`, `write`, `grep`,
`find` and `ls`. `ALL_TOOL_NAMES` uses that order. It identifies built-ins only;
custom executable tools keep their string names.

`create_tool` returns an executable `Tool` (`AgentTool`), while
`create_tool_definition::<C>` returns a typed `ToolDef<C>` variant. Both reject
any other spelling with `Unknown tool name: <name>`, preserving the input text.
They do not trim names or fold case.

The vector presets have fixed order:

- `create_coding_tools` / `create_coding_tool_definitions`: read, bash, edit, write.
- `create_read_only_tools` / `create_read_only_tool_definitions`: read, grep, find, ls.

`create_all_tools` and `create_all_tool_definitions` return an `IndexMap` keyed
by `ToolName`, in `ALL_TOOL_NAMES` order. Each invocation constructs new tools.

## Options and caller context

`ToolsOptions` groups the existing seven option records. Its default uses each
individual record's default; selecting a family forwards only its own member.
The factories take an explicit working-directory string, shell binary-directory
string and shared search `ToolsManager`. The shell uses the binary directory;
find and grep use the supplied search manager. Construction passes the working
directory to the individual factory without rewriting it.

For typed definitions, match the `ToolDef` variant to access the original
arguments, details and render state. Use `wrap_tool_definition` with a
`ToolContextFactory<C>` when execution needs fresh caller context. The executable
factories use the individual tools' no-context wrappers. The barrel adds no
execution, validation, rendering or acquisition policy.

Delegated behavior is documented in [read and definitions](tools-read.md),
[edit](tools-edit.md), [shell operations](tools-process.md) and
[file search](tools-search.md). Existing whole-file write and truncation helpers
are also named re-exports from `maestro_tools::tools`; they remain owned by their
individual modules.

## Renderer projections

[`ToolRendererBinding`](../crates/maestro-tools/src/renderer_binding.rs) clones only
renderer handles and shell policy, never executable definitions. Custom definitions
use `from_definition`; native `ToolDef::renderer_binding` projects factory raw slots.
Each row supplies a `ToolRendererState`: the same concrete state type shares its
handle across call/result slots and wrappers, while different types coexist.
JSON state starts as `{}`; other state types use `Default`.

The single result slot is `ToolRenderResult::Typed` or `Raw`. `invoke_native`
serializes present details only for raw callbacks; `invoke_raw` decodes present
details only for typed callbacks. Missing details never invoke conversion, and
present null stays present. Builtins select only rendered JSON fields, accepting
sparse metadata without changing execution records. Invalid selected field types
use that field's absent fallback; unknown fields stay unread.

A wrapper can borrow the existing map through `ToolRenderContext::renderer_state`,
select a native state handle, release the map borrow and use `with_state` to
delegate. This preserves the row's arguments, host effects and previous component.
The application root re-exports these renderer types for frontends.

# Native tool operations from a component

`ExtensionAPI::create_read_tool_definition(cwd, options)` returns a retained native
read definition. Construction does not read a file. Its metadata, schema,
execution and renderers come from the native tool owner. See [read](../tools-read.md)
for its file, image and output rules.

## Execute without an extension context

`ExtensionAPI::create_read_tool(cwd, options)` returns a `Tool`. Its `execute`
callback takes the call ID, JSON arguments, optional signal and optional progress
callback. It does not read a command or event context, even when called from one.

The executable keeps the native name, label, description, parameter schema,
preparation and execution mode. The parameter schema remains the shared
`ParameterSchema<SchemaCode>`, not a JSON string. Prompt fields and renderers
belong to definitions only.

## Wrap and register a read definition

```rust
use std::rc::Rc;
use maestro_extensions_wasm::{ExtensionAPI, ExtensionResult, ReadToolOptions};

fn register_read(api: &ExtensionAPI, cwd: String) -> ExtensionResult<()> {
    let mut read = api.create_read_tool_definition(cwd, ReadToolOptions::default())?;
    let native = read.render_call.take().ok_or("read call renderer absent")?;
    read.render_call = Some(Rc::new(move |args, theme, context, emitter| {
        let mut expanded = context.clone();
        expanded.expanded = true;
        native(args, theme, &expanded, emitter)
    }));
    api.register_tool(read)
}
```

Pass the received render context when delegating. Its opaque `native_row` keeps
the original native environment, state and invalidation capability. Public row
observations may be changed on a clone. Call arguments, result options and the
supplied theme remain independent of those observations.

The previous component is exactly the one passed by the wrapper: a native
component, an authored replacement or none. Returning a native component lets
its next rendering reuse that native object. Wrapping it in another `Component`
does not pretend the wrapper is the native object. Calling a native renderer
without `native_row` returns `native tool renderer requires a native row` and
produces no component.

Result renderers receive `ToolRenderResultLike`. Its flattened `result` keeps the
existing `AgentToolResult` content, details and termination fields; `is_error`
keeps the independent rendering observation, including absence. Execution still
returns `AgentToolResult`.

## Replace file observations

`ReadToolOptions` has independent `auto_resize_images` and `operations` fields.
The image fitting default is enabled. Omitting `operations` uses native file
access. Supplying `ReadOperations` replaces only these observations:

| Callback | Result |
| --- | --- |
| `access(absolute_path)` | Success or a diagnostic that stops the read. |
| `read_file(absolute_path)` | Original bytes, as `Vec<u8>`. |
| `detect_image_mime_type(absolute_path)` | Optional MIME text; omitting the callback, returning `None` or returning an empty string selects text. |

Callbacks return `OperationFuture` with `DiagnosticErrorInfo` failures. The native
owner resolves paths against the construction directory, checks cancellation,
orders the observations, decodes text and applies image policy. The active model
is read from the execution context, not captured during construction.

The definition and its callbacks may outlive the factory call and the callback
that constructed it. Keep the returned definition or its callback clones for as
long as they are needed; dropping the last consumer releases their native
resources. These operations require the native component host and a
`wasm32-wasip2` component. The in-process controlled adapter does not simulate
native factories.

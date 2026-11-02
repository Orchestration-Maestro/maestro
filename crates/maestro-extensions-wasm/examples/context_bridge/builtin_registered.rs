//! Custom native definitions retained through real tool registration and interception.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    EditToolOptions, ExtensionAPI, ExtensionCommandContext, ExtensionResult, ToolName, ToolsOptions,
};
use serde_json::{Value, json};
use std::rc::Rc;

/// Reuses the existing custom edit effects and delegates all matching and queueing.
pub(super) fn register(
    api: &ExtensionAPI,
    context: &ExtensionCommandContext,
    request: &str,
) -> ExtensionResult<()> {
    let request: Value = serde_json::from_str(request).map_err(|e| e.to_string())?;
    let cwd = request["cwd"].as_str().ok_or("missing registered cwd")?;
    let mut operations = super::builtin_edit::registration_operations(api.events());
    let access = operations.access;
    operations.access = Rc::new(move |path| {
        if super::builtin::ends_in(&path, "error.txt") {
            return Box::pin(async {
                Err(maestro_extensions_wasm::DiagnosticErrorInfo::error(
                    "authored registered failure é",
                ))
            });
        }
        access(path)
    });
    let read = operations.read_file;
    let context = context.clone();
    operations.read_file = Rc::new(move |path| {
        let (read, context) = (read.clone(), context.clone());
        Box::pin(async move {
            let gate = super::builtin::ends_in(&path, "gate.txt");
            let bytes = read(path).await?;
            if gate {
                context
                    .wait_for_idle()
                    .await
                    .map_err(maestro_extensions_wasm::DiagnosticErrorInfo::error)?;
            }
            Ok(bytes)
        })
    });
    let options = EditToolOptions {
        operations: Some(operations),
    };
    let mut definition = if request["aggregate"] == true {
        api.create_all_tool_definitions(
            cwd.into(),
            ToolsOptions {
                edit: options,
                ..Default::default()
            },
        )?
        .shift_remove(&ToolName::Edit)
        .ok_or("aggregate edit missing")?
    } else {
        api.create_edit_tool_definition(cwd.into(), options)?
    };
    intercept(&mut definition, api.events());
    definition.metadata.name = "registered-custom-edit".into();
    api.register_tool(definition)
}

/// Intercepts execution while retaining the native preparation and complete definition.
fn intercept(
    definition: &mut maestro_extensions_wasm::ToolDefinition,
    events: maestro_extensions_wasm::EventBus,
) {
    let execute = definition.execute.clone();
    definition.execute = Rc::new(move |id, args, signal, update, context| {
        let (execute, events) = (execute.clone(), events.clone());
        Box::pin(async move {
            if let Some(update) = &update {
                let progress = serde_json::from_value(
                    json!({"content":[{"type":"text","text":"intercepted"}]}),
                )
                .map_err(|e| e.to_string())?;
                update(progress)?;
            }
            let mut work = execute(id, args, signal, update, context);
            let ready = std::future::poll_fn(|cx| {
                std::task::Poll::Ready(match work.as_mut().poll(cx) {
                    std::task::Poll::Ready(result) => Some(result),
                    std::task::Poll::Pending => None,
                })
            })
            .await;
            events
                .emit("builtin-registered-admitted", Value::Null)
                .await?;
            if let Some(result) = ready {
                result
            } else {
                work.await
            }
        })
    });
}

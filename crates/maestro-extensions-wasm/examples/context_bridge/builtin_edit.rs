//! Native edit consumes authored diagnostic codes before the final error becomes text.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    EditOperations, EditToolOptions, ExtensionAPI, ExtensionCommandContext, ExtensionResult,
};
use serde::Deserialize;
use std::{cell::RefCell, rc::Rc};

/// Original edit operands and an independently selected callback failure.
#[derive(Deserialize)]
struct Request {
    /// Original source bytes.
    bytes: Vec<u8>,
    /// Original model arguments, not a narrowed edit record.
    args: serde_json::Value,
    /// Optional callback failure stage.
    fail: Option<String>,
    /// Optional text or numeric diagnostic code.
    code: Option<maestro_extensions_wasm::DiagnosticCode>,
    /// Optional callback to suspend until the native caller permits completion.
    gate: Option<String>,
    /// Selects the context-free projection.
    executable: bool,
}
/// Reached original callback operands.
type Calls = Rc<RefCell<Vec<serde_json::Value>>>;

/// Uses the real native preparation, validation and edit callbacks.
pub(super) async fn execute(
    api: &ExtensionAPI,
    context: ExtensionCommandContext,
    request: &str,
) -> ExtensionResult<String> {
    let request: Request = serde_json::from_str(request).map_err(|error| error.to_string())?;
    let calls = Calls::default();
    let args = request.args.clone();
    let executable = request.executable;
    let options = EditToolOptions {
        operations: Some(operations(request, calls.clone(), context.clone())),
    };
    let result = if executable {
        let tool = api.create_edit_tool("/virtual".into(), options)?;
        (tool.execute)("custom-edit".into(), args, None, None).await
    } else {
        let tool = api.create_edit_tool_definition("/virtual".into(), options)?;
        (tool.execute)("custom-edit".into(), args, None, None, (*context).clone()).await
    };
    let mut response = serde_json::json!({"calls":*calls.borrow()});
    match result {
        Ok(result) => {
            response["result"] = serde_json::to_value(result).map_err(|error| error.to_string())?;
        }
        Err(error) => response["error"] = serde_json::json!(error),
    }
    Ok(response.to_string())
}
/// Replaces effects only; matching, line endings and result text remain native-owned.
fn operations(request: Request, calls: Calls, context: ExtensionCommandContext) -> EditOperations {
    let observations = Rc::new(Observations {
        request,
        calls,
        context,
    });
    let access = observations.clone();
    let read = observations.clone();
    EditOperations {
        access: Rc::new(move |path| {
            let observed = access.clone();
            Box::pin(async move { observed.answer("access", &path).await })
        }),
        read_file: Rc::new(move |path| {
            let observed = read.clone();
            Box::pin(async move {
                observed.answer("read", &path).await?;
                Ok(observed.request.bytes.clone())
            })
        }),
        write_file: Rc::new(move |path, content| {
            let observed = observations.clone();
            Box::pin(async move {
                observed.answer("write", &path).await?;
                observed
                    .calls
                    .borrow_mut()
                    .push(serde_json::json!(["written", content]));
                Ok(())
            })
        }),
    }
}
/// Completion and failure behavior shared by the three supplied effects.
struct Observations {
    /// Original authored outcome choices.
    request: Request,
    /// Original reached operands.
    calls: Calls,
    /// Native completion gate supplied to this callback.
    context: ExtensionCommandContext,
}
impl Observations {
    /// Preserves error fields until the native edit consumer formats them.
    async fn answer(
        &self,
        stage: &str,
        path: &str,
    ) -> Result<(), maestro_extensions_wasm::DiagnosticErrorInfo> {
        self.calls
            .borrow_mut()
            .push(serde_json::json!([stage, path]));
        if self.request.gate.as_deref() == Some(stage) {
            self.context
                .wait_for_idle()
                .await
                .map_err(maestro_extensions_wasm::DiagnosticErrorInfo::error)?;
        }
        if self.request.fail.as_deref() == Some(stage) {
            return Err(maestro_extensions_wasm::DiagnosticErrorInfo {
                name: Some("EditFailure".into()),
                message: format!("refused {stage} 雪"),
                stack: Some("authored stack".into()),
                code: self.request.code.clone(),
            });
        }
        Ok(())
    }
}

/// Registers both preparation projections with observable effects on the real native bus.
pub(super) fn register(api: &ExtensionAPI, request: &str) -> ExtensionResult<()> {
    let request: serde_json::Value =
        serde_json::from_str(request).map_err(|error| error.to_string())?;
    let options = EditToolOptions {
        operations: Some(registration_operations(api.events())),
    };
    let mut definition = api.create_edit_tool_definition("/virtual".into(), options)?;
    if request["executable"] == true {
        let tool = api.create_edit_tool("/virtual".into(), EditToolOptions::default())?;
        definition.prepare_arguments = tool.prepare_arguments;
    }
    if request["prepareFailure"] == true {
        let prepare = definition
            .prepare_arguments
            .take()
            .ok_or("missing edit preparation")?;
        definition.prepare_arguments = Some(Rc::new(move |args, emitter| {
            prepare(args, emitter)?;
            Err("authored preparation failed".into())
        }));
    }
    definition.metadata.name = "builtin-edit".into();
    api.register_tool(definition)
}

/// Supplies byte observations whose complete reached effects are visible to the native caller.
pub(super) fn registration_operations(events: maestro_extensions_wasm::EventBus) -> EditOperations {
    let access = events;
    let read = access.clone();
    let write = access.clone();
    EditOperations {
        access: Rc::new(move |path| {
            let events = access.clone();
            Box::pin(async move {
                events
                    .emit("builtin-edit-effect", serde_json::json!(["access", path]))
                    .await
                    .map_err(maestro_extensions_wasm::DiagnosticErrorInfo::error)
            })
        }),
        read_file: Rc::new(move |path| {
            let events = read.clone();
            Box::pin(async move {
                events
                    .emit("builtin-edit-effect", serde_json::json!(["read", path]))
                    .await
                    .map_err(maestro_extensions_wasm::DiagnosticErrorInfo::error)?;
                Ok(b"a\nc\n".to_vec())
            })
        }),
        write_file: Rc::new(move |path, content| {
            let events = write.clone();
            Box::pin(async move {
                events
                    .emit(
                        "builtin-edit-effect",
                        serde_json::json!(["write", path, content]),
                    )
                    .await
                    .map_err(maestro_extensions_wasm::DiagnosticErrorInfo::error)
            })
        }),
    }
}

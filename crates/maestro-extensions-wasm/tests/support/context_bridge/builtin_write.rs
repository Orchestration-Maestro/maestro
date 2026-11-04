//! Replacement writes leave path, ordering and byte-count policy with the native owner.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionCommandContext, ExtensionResult, WriteOperations, WriteToolOptions,
};
use serde::Deserialize;
use std::{cell::RefCell, rc::Rc};

/// Supplied write data and independently selected completion behavior.
#[derive(Deserialize)]
struct Request {
    /// Original content, including embedded NUL and Unicode.
    content: String,
    /// Selects the context-free projection.
    executable: bool,
    /// Operation to reject.
    fail: Option<String>,
    /// Operation to suspend until the native caller permits completion.
    gate: Option<String>,
    /// Optional already-aborted or arbitrary relayed signal.
    signal: Option<String>,
}
/// Original reached operands, without a filesystem implementation in the guest.
type Calls = Rc<RefCell<Vec<serde_json::Value>>>;

/// Executes a native write with retained authored effects.
pub(super) async fn execute(
    api: &ExtensionAPI,
    context: ExtensionCommandContext,
    request: &str,
) -> ExtensionResult<String> {
    let request: Request = serde_json::from_str(request).map_err(|error| error.to_string())?;
    let calls = Calls::default();
    let (signal, notified) = match request.signal.as_deref() {
        Some("preaborted") => (Some(super::exec::preaborted()), Some(Rc::default())),
        Some("relay") => {
            let (signal, notified) =
                super::exec::relay(context.signal()?.ok_or("missing signal")?, None);
            (Some(signal), Some(notified))
        }
        None => (None, None),
        Some(_) => return Err("unknown write signal".into()),
    };
    let args = serde_json::json!({"path":"parent/雪.txt","content":request.content});
    let executable = request.executable;
    let options = WriteToolOptions {
        operations: Some(operations(request, calls.clone(), context.clone())),
    };
    let result = if executable {
        let tool = api.create_write_tool("/virtual".into(), options)?;
        (tool.execute)("custom-write".into(), args, signal, None).await
    } else {
        let tool = api.create_write_tool_definition("/virtual".into(), options)?;
        (tool.execute)(
            "custom-write".into(),
            args,
            signal,
            None,
            (*context).clone(),
        )
        .await
    };
    let mut response = serde_json::json!({"calls":*calls.borrow()});
    match result {
        Ok(result) => {
            response["result"] = serde_json::to_value(result).map_err(|error| error.to_string())?;
        }
        Err(error) => response["error"] = serde_json::json!(error),
    }
    if let Some(notified) = notified {
        response["notified"] = serde_json::json!(notified.get());
    }
    Ok(response.to_string())
}

/// Records entry operands and checks the completed parent operation at the write boundary.
fn operations(request: Request, calls: Calls, context: ExtensionCommandContext) -> WriteOperations {
    let observations = Rc::new(Observations {
        request,
        calls,
        context,
        made: RefCell::new(false),
    });
    let observed = observations.clone();
    WriteOperations {
        mkdir: Rc::new(move |path| {
            let observed = observed.clone();
            Box::pin(async move {
                observed
                    .calls
                    .borrow_mut()
                    .push(serde_json::json!(["mkdir", path]));
                observed.answer("mkdir").await?;
                *observed.made.borrow_mut() = true;
                Ok(())
            })
        }),
        write_file: Rc::new(move |path, content| {
            let observed = observations.clone();
            Box::pin(async move {
                observed.calls.borrow_mut().push(serde_json::json!([
                    "write",
                    path,
                    content,
                    *observed.made.borrow()
                ]));
                observed.answer("write").await
            })
        }),
    }
}
/// Completion gates and diagnostics belong to the authored callback, not the native factory.
struct Observations {
    /// Selected callback outcomes.
    request: Request,
    /// Reached original operands.
    calls: Calls,
    /// Supplied context owning the completion gate.
    context: ExtensionCommandContext,
    /// Whether mkdir has completed successfully.
    made: RefCell<bool>,
}
impl Observations {
    /// Waits or fails only at the selected stage.
    async fn answer(
        &self,
        stage: &str,
    ) -> Result<(), maestro_extensions_wasm::DiagnosticErrorInfo> {
        if self.request.gate.as_deref() == Some(stage) {
            self.context
                .wait_for_idle()
                .await
                .map_err(maestro_extensions_wasm::DiagnosticErrorInfo::error)?;
        }
        if self.request.fail.as_deref() == Some(stage) {
            return Err(maestro_extensions_wasm::DiagnosticErrorInfo {
                name: Some("WriteFailure".into()),
                message: format!("refused {stage} 雪"),
                stack: Some("authored stack".into()),
                code: Some(maestro_extensions_wasm::DiagnosticCode::Number(17.0)),
            });
        }
        Ok(())
    }
}

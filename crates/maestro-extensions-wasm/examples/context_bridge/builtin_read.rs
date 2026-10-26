//! An author replaces file observations while the native owner keeps read policy.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionCommandContext, ExtensionResult, ReadOperations, ReadToolOptions,
};
use serde::Deserialize;
use std::{cell::RefCell, rc::Rc};

/// Controlled observations supplied by the runtime witness.
#[derive(Deserialize)]
struct Request {
    /// Unmodified bytes the replacement file returns.
    bytes: Vec<u8>,
    /// Whether the MIME callback exists at all.
    detect: bool,
    /// Its independent optional result.
    mime: Option<String>,
    /// Native image fitting choice.
    resize: Option<bool>,
    /// Optional operation to reject with a structured diagnostic.
    fail: Option<String>,
    /// Uses the four-argument executable instead of the context-bearing definition.
    #[serde(default)]
    executable: bool,
    /// Parks after construction so the caller can change the model before execution.
    #[serde(default)]
    before: bool,
    /// Operation whose future parks on a native completion gate.
    gate: Option<String>,
    /// Optional arbitrary caller signal, either already aborted or relaying a later abort.
    signal: Option<String>,
}
/// Records the native arguments to every supplied observation.
type Calls = Rc<RefCell<Vec<serde_json::Value>>>;

/// Invokes the returned definition after its synchronous factory has returned.
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
        Some(_) => return Err("unknown read signal".into()),
    };
    let (prepares, result) = run(api, &context, request, &calls, signal).await?;
    let mut response = serde_json::json!({"calls":*calls.borrow(),"prepares":prepares});
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

/// Constructs the selected native projection, then invokes it after any requested gate.
async fn run(
    api: &ExtensionAPI,
    context: &ExtensionCommandContext,
    request: Request,
    calls: &Calls,
    signal: Option<maestro_extensions_wasm::AbortSignal>,
) -> ExtensionResult<(
    bool,
    ExtensionResult<maestro_extensions_wasm::AgentToolResult>,
)> {
    let executable = request.executable;
    let before = request.before;
    let options = ReadToolOptions {
        auto_resize_images: request.resize,
        operations: Some(operations(request, calls, context.clone())),
    };
    let args = serde_json::json!({"path":"雪.bin"});
    let result = if executable {
        let tool = api.create_read_tool("/virtual".into(), options)?;
        if before {
            context.wait_for_idle().await?;
        }
        (
            tool.prepare_arguments.is_some(),
            (tool.execute)("custom-read".into(), args, signal, None).await,
        )
    } else {
        let tool = api.create_read_tool_definition("/virtual".into(), options)?;
        if before {
            context.wait_for_idle().await?;
        }
        (
            tool.prepare_arguments.is_some(),
            (tool.execute)(
                "custom-read".into(),
                args,
                signal,
                None,
                (**context).clone(),
            )
            .await,
        )
    };
    Ok(result)
}

/// Makes the complete replacement operation record, preserving an omitted detector.
fn operations(request: Request, calls: &Calls, context: ExtensionCommandContext) -> ReadOperations {
    let observations = Rc::new(Observations {
        calls: calls.clone(),
        fail: request.fail,
        gate: request.gate,
        context,
    });
    let observed = observations.clone();
    let read_file = Rc::new(move |path: String| {
        let (observed, bytes) = (observed.clone(), request.bytes.clone());
        Box::pin(async move { observed.answer("read", &path, bytes).await })
            as maestro_extensions_wasm::OperationFuture<'static, Vec<u8>>
    });
    let observed = observations.clone();
    let access = Rc::new(move |path: String| {
        let observed = observed.clone();
        Box::pin(async move { observed.answer("access", &path, ()).await })
            as maestro_extensions_wasm::OperationFuture<'static, ()>
    });
    let detect_image_mime_type = request.detect.then(|| {
        let observed = observations;
        Rc::new(move |path: String| {
            let (observed, mime) = (observed.clone(), request.mime.clone());
            Box::pin(async move { observed.answer("mime", &path, mime).await })
                as maestro_extensions_wasm::OperationFuture<'static, Option<String>>
        }) as maestro_extensions_wasm::DetectImageMimeType
    });
    ReadOperations {
        read_file,
        access,
        detect_image_mime_type,
    }
}

/// One authored trace and independently chosen failure stage.
struct Observations {
    /// Original operands in reached order.
    calls: Calls,
    /// Operation whose response is rejected.
    fail: Option<String>,
    /// Operation that must await native permission to answer.
    gate: Option<String>,
    /// Original supplied context owning the completion gate.
    context: ExtensionCommandContext,
}
impl Observations {
    /// Records the operation before answering with its supplied outcome.
    async fn answer<T>(
        &self,
        stage: &str,
        path: &str,
        value: T,
    ) -> Result<T, maestro_extensions_wasm::DiagnosticErrorInfo> {
        self.calls
            .borrow_mut()
            .push(serde_json::json!([stage, path]));
        if self.gate.as_deref() == Some(stage) {
            self.context
                .wait_for_idle()
                .await
                .map_err(|error| maestro_extensions_wasm::DiagnosticErrorInfo::error(&error))?;
        }
        if self.fail.as_deref() == Some(stage) {
            return Err(maestro_extensions_wasm::DiagnosticErrorInfo {
                name: Some("ReadFailure".into()),
                message: format!("refused {stage} 雪"),
                stack: Some("authored stack".into()),
                code: Some(maestro_extensions_wasm::DiagnosticCode::Number(17.0)),
            });
        }
        Ok(value)
    }
}

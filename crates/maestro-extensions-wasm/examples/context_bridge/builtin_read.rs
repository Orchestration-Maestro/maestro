//! An author replaces file observations while the native owner keeps read policy.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionContext, ExtensionResult, ReadOperations, ReadToolOptions,
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
}
/// Records the native arguments to every supplied observation.
type Calls = Rc<RefCell<Vec<serde_json::Value>>>;

/// Invokes the returned definition after its synchronous factory has returned.
pub(super) async fn execute(
    api: &ExtensionAPI,
    context: ExtensionContext,
    request: &str,
) -> ExtensionResult<String> {
    let request: Request = serde_json::from_str(request).map_err(|error| error.to_string())?;
    let calls = Calls::default();
    let executable = request.executable;
    let options = ReadToolOptions {
        auto_resize_images: request.resize,
        operations: Some(operations(request, &calls)),
    };
    let args = serde_json::json!({"path":"雪.bin"});
    let (prepares, result) = if executable {
        let tool = api.create_read_tool("/virtual".into(), options)?;
        (
            tool.prepare_arguments.is_some(),
            (tool.execute)("custom-read".into(), args, None, None).await,
        )
    } else {
        let tool = api.create_read_tool_definition("/virtual".into(), options)?;
        (
            tool.prepare_arguments.is_some(),
            (tool.execute)("custom-read".into(), args, None, None, context).await,
        )
    };
    let mut response = serde_json::json!({"calls":*calls.borrow(),"prepares":prepares});
    match result {
        Ok(result) => {
            response["result"] = serde_json::to_value(result).map_err(|error| error.to_string())?;
        }
        Err(error) => response["error"] = serde_json::json!(error),
    }
    Ok(response.to_string())
}

/// Makes the complete replacement operation record, preserving an omitted detector.
fn operations(request: Request, calls: &Calls) -> ReadOperations {
    let observations = Rc::new(Observations {
        calls: calls.clone(),
        fail: request.fail,
    });
    let observed = observations.clone();
    let read_file = Rc::new(move |path: String| {
        let result = observed.answer("read", &path, request.bytes.clone());
        Box::pin(std::future::ready(result))
            as maestro_extensions_wasm::OperationFuture<'static, Vec<u8>>
    });
    let observed = observations.clone();
    let access = Rc::new(move |path: String| {
        let result = observed.answer("access", &path, ());
        Box::pin(std::future::ready(result))
            as maestro_extensions_wasm::OperationFuture<'static, ()>
    });
    let detect_image_mime_type = request.detect.then(|| {
        let observed = observations;
        Rc::new(move |path: String| {
            let result = observed.answer("mime", &path, request.mime.clone());
            Box::pin(std::future::ready(result))
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
}
impl Observations {
    /// Records the operation before answering with its supplied outcome.
    fn answer<T>(
        &self,
        stage: &str,
        path: &str,
        value: T,
    ) -> Result<T, maestro_extensions_wasm::DiagnosticErrorInfo> {
        self.calls
            .borrow_mut()
            .push(serde_json::json!([stage, path]));
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

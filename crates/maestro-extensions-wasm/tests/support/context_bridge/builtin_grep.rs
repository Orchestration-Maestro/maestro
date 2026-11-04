//! Native search keeps its program owner while authored callbacks supply context text.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    DiagnosticErrorInfo, ExtensionAPI, ExtensionCommandContext, ExtensionResult, GrepOperations,
    GrepToolOptions,
};
use serde::Deserialize;
use std::{cell::RefCell, rc::Rc};

/// Selected native search operands and original callback observations.
#[derive(Deserialize)]
struct Request {
    /// Root passed unchanged to the native factory.
    cwd: String,
    /// Original search arguments.
    args: serde_json::Value,
    /// Whether to replace filesystem observations.
    custom: bool,
    /// Supplied root directory observation.
    directory: bool,
    /// Supplied context text, independent of the actual searched file.
    text: String,
    /// Callback to reject.
    fail: Option<String>,
    /// Callback to suspend.
    gate: Option<String>,
    /// Whether to use the context-free execution projection.
    executable: bool,
}
/// Executes through the original program acquisition and search implementation.
pub(super) async fn execute(
    api: &ExtensionAPI,
    context: ExtensionCommandContext,
    request: &str,
) -> ExtensionResult<String> {
    let request: Request = serde_json::from_str(request).map_err(|error| error.to_string())?;
    let calls = Rc::new(RefCell::new(Vec::new()));
    let (cwd, args, executable) = (
        request.cwd.clone(),
        request.args.clone(),
        request.executable,
    );
    let options = GrepToolOptions {
        operations: request
            .custom
            .then(|| operations(request, calls.clone(), context.clone())),
    };
    let result = if executable {
        let tool = api.create_grep_tool(cwd, options)?;
        (tool.execute)("custom-grep".into(), args, None, None).await
    } else {
        let tool = api.create_grep_tool_definition(cwd, options)?;
        (tool.execute)("custom-grep".into(), args, None, None, (*context).clone()).await
    };
    Ok(match result {
        Ok(result) => serde_json::json!({"calls":*calls.borrow(),"result":result}),
        Err(error) => serde_json::json!({"calls":*calls.borrow(),"error":error}),
    }
    .to_string())
}
/// Keeps context observations callable after constructor return.
fn operations(
    request: Request,
    calls: Rc<RefCell<Vec<serde_json::Value>>>,
    context: ExtensionCommandContext,
) -> GrepOperations {
    let request = Rc::new(request);
    let (observed, trace, ctx) = (request.clone(), calls.clone(), context.clone());
    GrepOperations {
        is_directory: Rc::new(move |path| {
            let (request, calls, context) = (observed.clone(), trace.clone(), ctx.clone());
            Box::pin(async move {
                observe(&request, &calls, &context, "directory", path).await?;
                Ok(request.directory)
            })
        }),
        read_file: Rc::new(move |path| {
            let (request, calls, context) = (request.clone(), calls.clone(), context.clone());
            Box::pin(async move {
                observe(&request, &calls, &context, "read", path).await?;
                Ok(request.text.clone())
            })
        }),
    }
}
/// Reads a real supplied context before each ready or pending authored observation.
async fn observe(
    request: &Request,
    calls: &Rc<RefCell<Vec<serde_json::Value>>>,
    context: &ExtensionCommandContext,
    stage: &str,
    path: String,
) -> Result<(), DiagnosticErrorInfo> {
    let cwd = context.cwd().map_err(DiagnosticErrorInfo::error)?;
    calls
        .borrow_mut()
        .push(serde_json::json!([stage, path, cwd]));
    if request.gate.as_deref() == Some(stage) {
        context
            .wait_for_idle()
            .await
            .map_err(DiagnosticErrorInfo::error)?;
    }
    if request.fail.as_deref() == Some(stage) {
        return Err(DiagnosticErrorInfo::error(format!("refused {stage} 雪")));
    }
    Ok(())
}

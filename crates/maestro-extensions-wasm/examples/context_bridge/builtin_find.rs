//! Authored find observations are consumed by the original native finder.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    DiagnosticErrorInfo, ExtensionAPI, ExtensionCommandContext, ExtensionResult, FindOperations,
    FindToolOptions,
};
use serde::Deserialize;
use std::{cell::RefCell, rc::Rc};

/// Selected observations, execution projection and native arguments.
#[derive(Deserialize)]
struct Request {
    /// Original find arguments.
    args: serde_json::Value,
    /// Authored existence result.
    exists: bool,
    /// Ordered relative or absolute paths.
    paths: Vec<String>,
    /// Effect to fail with an authored diagnostic.
    fail: Option<String>,
    /// Effect to suspend until the caller permits completion.
    gate: Option<String>,
    /// Whether to use the context-free executable.
    executable: bool,
}
/// Executes the selected original factory and returns its complete observation.
pub(super) async fn execute(
    api: &ExtensionAPI,
    context: ExtensionCommandContext,
    request: &str,
) -> ExtensionResult<String> {
    let request: Request = serde_json::from_str(request).map_err(|error| error.to_string())?;
    let calls = Rc::new(RefCell::new(Vec::new()));
    let args = request.args.clone();
    let executable = request.executable;
    let options = FindToolOptions {
        operations: Some(operations(request, calls.clone(), context.clone())),
    };
    let result = if executable {
        let tool = api.create_find_tool("/virtual".into(), options)?;
        (tool.execute)("custom-find".into(), args, None, None).await
    } else {
        let tool = api.create_find_tool_definition("/virtual".into(), options)?;
        (tool.execute)("custom-find".into(), args, None, None, (*context).clone()).await
    };
    Ok(match result {
        Ok(result) => serde_json::json!({"calls":*calls.borrow(),"result":result}),
        Err(error) => serde_json::json!({"calls":*calls.borrow(),"error":error}),
    }
    .to_string())
}
/// Retains callback operands without implementing search or path selection.
fn operations(
    request: Request,
    calls: Rc<RefCell<Vec<serde_json::Value>>>,
    context: ExtensionCommandContext,
) -> FindOperations {
    let request = Rc::new(request);
    let (observed, trace, ctx) = (request.clone(), calls.clone(), context.clone());
    FindOperations {
        exists: Rc::new(move |path| {
            let (request, calls, context) = (observed.clone(), trace.clone(), ctx.clone());
            Box::pin(async move {
                calls.borrow_mut().push(serde_json::json!(["exists", path]));
                answer(&request, &context, "exists").await?;
                Ok(request.exists)
            })
        }),
        glob: Rc::new(move |pattern, root, options| {
            let (request, calls, context) = (request.clone(), calls.clone(), context.clone());
            Box::pin(async move {
                calls.borrow_mut().push(serde_json::json!([
                    "glob",pattern,root,{"ignore":options.ignore,"limit":options.limit}
                ]));
                answer(&request, &context, "glob").await?;
                Ok(request.paths.clone())
            })
        }),
    }
}
/// Ready and pending failures retain the original authored diagnostic.
async fn answer(
    request: &Request,
    context: &ExtensionCommandContext,
    stage: &str,
) -> Result<(), DiagnosticErrorInfo> {
    if request.gate.as_deref() == Some(stage) {
        context
            .wait_for_idle()
            .await
            .map_err(DiagnosticErrorInfo::error)?;
    }
    if request.fail.as_deref() == Some(stage) {
        return Err(DiagnosticErrorInfo {
            name: Some("FindFailure".into()),
            message: format!("refused {stage} 雪"),
            stack: None,
            code: Some(maestro_extensions_wasm::DiagnosticCode::Number(17.0)),
        });
    }
    Ok(())
}

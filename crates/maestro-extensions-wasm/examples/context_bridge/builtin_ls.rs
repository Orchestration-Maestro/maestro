//! Authored directory observations consumed by the original native listing owner.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    DiagnosticErrorInfo, ExtensionAPI, ExtensionCommandContext, ExtensionResult, LsOperations,
    LsToolOptions,
};
use serde::Deserialize;
use std::{cell::RefCell, rc::Rc};

/// Original arguments and selected callback observations.
#[derive(Deserialize)]
struct Request {
    /// Native listing arguments.
    args: serde_json::Value,
    /// Root existence.
    exists: bool,
    /// Root directory observation.
    directory: bool,
    /// Original unsorted entry names.
    names: Vec<String>,
    /// Callback phase to fail.
    fail: Option<String>,
    /// Callback phase to suspend.
    gate: Option<String>,
    /// Selects the context-free projection.
    executable: bool,
}
/// Executes through the facade and returns the closed effect transcript and complete outcome.
pub(super) async fn execute(
    api: &ExtensionAPI,
    context: ExtensionCommandContext,
    request: &str,
) -> ExtensionResult<String> {
    let request: Request = serde_json::from_str(request).map_err(|error| error.to_string())?;
    let calls = Rc::new(RefCell::new(Vec::new()));
    let args = request.args.clone();
    let executable = request.executable;
    let options = LsToolOptions {
        operations: Some(operations(request, calls.clone(), context.clone())),
    };
    let result = if executable {
        let tool = api.create_ls_tool("/virtual".into(), options)?;
        (tool.execute)("custom-ls".into(), args, None, None).await
    } else {
        let tool = api.create_ls_tool_definition("/virtual".into(), options)?;
        (tool.execute)("custom-ls".into(), args, None, None, (*context).clone()).await
    };
    Ok(match result {
        Ok(result) => serde_json::json!({"calls":*calls.borrow(),"result":result}),
        Err(error) => serde_json::json!({"calls":*calls.borrow(),"error":error}),
    }
    .to_string())
}
/// Retains only observations, with no authored sorting, path or truncation policy.
fn operations(
    request: Request,
    calls: Rc<RefCell<Vec<serde_json::Value>>>,
    context: ExtensionCommandContext,
) -> LsOperations {
    let request = Rc::new(request);
    let (exists, trace, ctx) = (request.clone(), calls.clone(), context.clone());
    let (stat, stats, stat_ctx) = (request.clone(), calls.clone(), context.clone());
    LsOperations {
        exists: Rc::new(move |path| {
            let (request, calls, context) = (exists.clone(), trace.clone(), ctx.clone());
            Box::pin(async move {
                calls.borrow_mut().push(serde_json::json!(["exists", path]));
                answer(&request, &context, "exists").await?;
                Ok(request.exists)
            })
        }),
        stat: Rc::new(move |path| {
            let (request, calls, context) = (stat.clone(), stats.clone(), stat_ctx.clone());
            Box::pin(async move {
                calls.borrow_mut().push(serde_json::json!(["stat", path]));
                let stage = if super::builtin::ends_in(&path, "virtual/雪") {
                    "stat"
                } else if super::builtin::ends_in(&path, "skip") {
                    "skip"
                } else {
                    "entry"
                };
                answer(&request, &context, stage).await?;
                Ok(if stage == "stat" {
                    request.directory
                } else {
                    super::builtin::ends_in(&path, "Dir")
                })
            })
        }),
        readdir: Rc::new(move |path| {
            let (request, calls, context) = (request.clone(), calls.clone(), context.clone());
            Box::pin(async move {
                calls
                    .borrow_mut()
                    .push(serde_json::json!(["readdir", path]));
                answer(&request, &context, "readdir").await?;
                Ok(request.names.clone())
            })
        }),
    }
}
/// Ready and pending diagnostics keep their original fields until consumed by the native owner.
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
    if request.fail.as_deref() == Some(stage) || stage == "skip" {
        return Err(DiagnosticErrorInfo {
            name: Some("ListingFailure".into()),
            message: format!("refused {stage} 雪"),
            stack: None,
            code: Some(maestro_extensions_wasm::DiagnosticCode::Number(17.0)),
        });
    }
    Ok(())
}

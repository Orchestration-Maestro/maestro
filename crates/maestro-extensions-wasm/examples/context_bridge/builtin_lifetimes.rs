//! Native returned-tool aliases retain callback captures until their final consumer releases.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    DiagnosticErrorInfo, ExtensionAPI, ExtensionCommandContext, ExtensionResult, ReadOperations,
    ReadToolOptions, Tool, ToolsOptions,
};
use std::{cell::RefCell, rc::Rc};

thread_local! {
    /// Author operands only, retained across independently admitted commands.
    static KEPT: RefCell<Vec<Rc<Tool>>> = const { RefCell::new(Vec::new()) };
}

/// Captures one destructor receipt shared by all aliases of a supplied operation.
fn options(
    api: &ExtensionAPI,
    context: &ExtensionCommandContext,
    label: &'static str,
) -> ReadToolOptions {
    let capture = Rc::new(super::stream_lifetimes::Released::new(api, label));
    let context = context.clone();
    ReadToolOptions {
        operations: Some(ReadOperations {
            access: Rc::new(|_| Box::pin(async { Ok(()) })),
            detect_image_mime_type: None,
            read_file: Rc::new(move |_| {
                let (capture, context) = (capture.clone(), context.clone());
                Box::pin(async move {
                    if label == "first-capture" {
                        context
                            .wait_for_idle()
                            .await
                            .map_err(DiagnosticErrorInfo::error)?;
                    }
                    let _ = &capture;
                    if label == "error-capture" {
                        Err(DiagnosticErrorInfo::error("owned callback failure 雪"))
                    } else {
                        Ok(label.as_bytes().to_vec())
                    }
                })
            }),
        }),
        ..ReadToolOptions::default()
    }
}

/// Factory captures outlive this command; rejected admission rolls back its own capture.
fn create(api: &ExtensionAPI, context: &ExtensionCommandContext) -> ExtensionResult<()> {
    let first = Rc::new(api.create_read_tool("/".into(), options(api, context, "first-capture"))?);
    let second =
        Rc::new(api.create_read_tool("/".into(), options(api, context, "second-capture"))?);
    let error = Rc::new(api.create_read_tool("/".into(), options(api, context, "error-capture"))?);
    let replaced = KEPT.replace(vec![first.clone(), first, second, error]);
    drop(replaced);
    let rejected = api.create_tool(
        "not-built-in",
        "/".into(),
        ToolsOptions {
            read: options(api, context, "rejected-capture"),
            ..ToolsOptions::default()
        },
    );
    if rejected.err().as_deref() != Some("Unknown tool name: not-built-in") {
        return Err("factory rejection changed".into());
    }
    Ok(())
}

/// Drops outside the fixture borrow, as native destruction can immediately reenter the guest.
fn release(index: usize) -> ExtensionResult<()> {
    let tool = KEPT
        .with_borrow_mut(|tools| (index < tools.len()).then(|| tools.remove(index)))
        .ok_or("missing retained tool alias")?;
    drop(tool);
    Ok(())
}

/// Executes after construction returns, including after all external aliases are released.
async fn execute(index: usize) -> ExtensionResult<String> {
    let tool = KEPT
        .with_borrow(|tools| tools.get(index).cloned())
        .ok_or("missing tool owner")?;
    let result = (tool.execute)(
        "lifetime".into(),
        serde_json::json!({"path":"unused.txt"}),
        None,
        None,
    )
    .await?;
    serde_json::to_string(&result).map_err(|error| error.to_string())
}

/// Keeps admission, pending work, errors and final-alias release separate and observable.
pub(super) async fn command(
    api: &ExtensionAPI,
    context: &ExtensionCommandContext,
    action: &str,
) -> ExtensionResult<String> {
    match action {
        "create" => create(api, context)?,
        "first" | "remaining" => return execute(0).await,
        "second" => return execute(2).await,
        "error" => return execute(3).await,
        "drop-first" | "drop-first-alias" | "drop-second" | "drop-error" => release(0)?,
        "barrier" => {}
        _ => return Err("unknown tool lifetime action".into()),
    }
    Ok(action.into())
}

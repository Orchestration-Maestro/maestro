//! Progress observers on original native shell tools.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    AgentToolUpdateCallback, BashExecResult, BashOperations, BashToolOptions, ExtensionAPI,
    ExtensionCommandContext, ExtensionResult,
};
use serde::Deserialize;
use std::{cell::RefCell, rc::Rc};

/// Selects the original executable projection and supplied output stream.
#[derive(Deserialize)]
struct Request {
    /// Whether to omit definition-only context.
    executable: bool,
    /// Literal working directory.
    cwd: String,
    /// Uses the real local backend when supplied.
    command: Option<String>,
    /// Arbitrarily split raw output bytes.
    chunks: Vec<Vec<u8>>,
    /// Fails the initial, later, or final coalesced observer invocation.
    fail: Option<String>,
    /// Rejects the supplied operation after all raw chunks have reached the native owner.
    operation_error: Option<String>,
}

/// Observes progress at callback time, before native settlement.
pub(super) async fn execute(
    api: &ExtensionAPI,
    context: ExtensionCommandContext,
    request: &str,
) -> ExtensionResult<String> {
    let request: Request = serde_json::from_str(request).map_err(|error| error.to_string())?;
    let updates = Rc::new(RefCell::new(Vec::new()));
    let seen = updates.clone();
    let update: AgentToolUpdateCallback = Rc::new(move |value| {
        let first = seen.borrow().is_empty();
        seen.borrow_mut().push(value);
        if request.fail.as_deref() == Some(if first { "initial" } else { "later" })
            || (request.fail.as_deref() == Some("final") && seen.borrow().len() == 3)
        {
            return Err("progress observer rejected é".into());
        }
        Ok(())
    });
    let entered = Rc::new(RefCell::new(0));
    let calls = entered.clone();
    let signals = Rc::new(RefCell::new(Vec::new()));
    let received = signals.clone();
    let operations = BashOperations {
        exec: Rc::new(move |_, _, options| {
            *calls.borrow_mut() += 1;
            received.borrow_mut().push(options.signal.is_some());
            let chunks = request.chunks.clone();
            let error = request.operation_error.clone();
            Box::pin(async move {
                for chunk in chunks {
                    (options.on_data)(&chunk)?;
                }
                if let Some(error) = error {
                    return Err(error);
                }
                Ok(BashExecResult {
                    exit_code: Some(0.0),
                })
            })
        }),
    };
    let options = BashToolOptions {
        operations: request.command.is_none().then_some(operations),
        ..BashToolOptions::default()
    };
    let args = serde_json::json!({"command":request.command.unwrap_or_else(|| "supplied".into())});
    let result = if request.executable {
        let tool = api.create_bash_tool(request.cwd, options)?;
        (tool.execute)("progress".into(), args, None, Some(update)).await
    } else {
        let tool = api.create_bash_tool_definition(request.cwd, options)?;
        (tool.execute)(
            "progress".into(),
            args,
            None,
            Some(update),
            (*context).clone(),
        )
        .await
    };
    Ok(serde_json::json!({"updates":*updates.borrow(),"entered":*entered.borrow(),"signals":*signals.borrow(),"result":result}).to_string())
}

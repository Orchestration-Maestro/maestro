//! HTTP operations through the ordinary author facade and retained response.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    CommandOptions, ExtensionAPI, ExtensionResult,
    http::{Request, Response},
    io::NativeIo,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// Literal request selections supplied to the component command.
#[derive(Deserialize)]
struct Fetch {
    /// Supplied destination.
    url: String,
    /// Omitted uses GET.
    method: Option<String>,
    /// Exact supplied header pairs.
    #[serde(default)]
    headers: Vec<(String, String)>,
    /// Omitted and present-empty stay distinct.
    body: Option<Vec<u8>>,
}
/// Retains the original native response across callbacks.
#[derive(Default)]
struct Slot(RefCell<Option<Rc<dyn Response>>>);

/// Registers the retained HTTP command without a second transport.
pub(super) fn register(api: &ExtensionAPI, io: Rc<dyn NativeIo>) -> ExtensionResult<()> {
    let slot = Rc::new(Slot::default());
    api.register_command(
        "native-http",
        CommandOptions {
            description: None,
            get_argument_completions: None,
            handler: Rc::new(move |args, context| {
                let (io, slot) = (Rc::clone(&io), Rc::clone(&slot));
                Box::pin(async move {
                    let result = execute(io, &slot, &args).await;
                    let reply = match result {
                        Ok(value) => json!({"value":value}),
                        Err(error) => json!({"error":error}),
                    };
                    context.ui()?.set_title(&format!("NATIVE-IO:{reply}"))
                })
            }),
        },
    )
}
/// Admits a request and observes its actual native status.
async fn execute(io: Rc<dyn NativeIo>, slot: &Slot, args: &str) -> ExtensionResult<Value> {
    let action: Value = serde_json::from_str(args).map_err(|e| e.to_string())?;
    if action["op"] == "read" {
        let response = slot.0.borrow().clone().ok_or("no retained response")?;
        return Ok(json!(
            response.body().ok_or("no response body")?.read().await?
        ));
    }
    let request: Fetch = serde_json::from_value(action).map_err(|e| e.to_string())?;
    let response = io
        .fetch(Request {
            url: request.url,
            method: request.method,
            headers: request.headers,
            body: request.body,
            signal: None,
        })
        .await?;
    let value = json!({"status":response.status(), "ok":response.ok(),
        "status_text":response.status_text(), "headers":response.headers(),
        "body":response.body().is_some()});
    *slot.0.borrow_mut() = Some(response);
    Ok(value)
}

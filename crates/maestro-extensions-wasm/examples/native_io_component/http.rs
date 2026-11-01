//! HTTP operations through the ordinary author facade and retained response.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    AbortSignal, CommandOptions, ExtensionAPI, ExtensionFuture, ExtensionResult,
    http::{Request, Response},
    io::NativeIo,
    streams::Readable,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc, task::Poll};

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
struct Slot {
    /// Original response across commands.
    response: RefCell<Option<Rc<dyn Response>>>,
    /// A body alias independent of response ownership.
    body: RefCell<Option<Rc<dyn Readable>>>,
    /// Original active-run cancellation, never a replacement controller.
    signal: RefCell<Option<AbortSignal>>,
    /// Request to start when the next run supplies its signal.
    armed: RefCell<Option<Fetch>>,
    /// One operation admitted before its creating callback returns.
    pending: RefCell<Option<ExtensionFuture<'static, Value>>>,
}

/// Registers the retained HTTP command without a second transport.
pub(super) fn register(api: &ExtensionAPI, io: Rc<dyn NativeIo>) -> ExtensionResult<()> {
    let slot = Rc::new(Slot::default());
    let (commands, retained) = (Rc::clone(&io), Rc::clone(&slot));
    api.register_command(
        "native-http",
        CommandOptions {
            description: None,
            get_argument_completions: None,
            handler: Rc::new(move |args, context| {
                let (io, slot) = (Rc::clone(&commands), Rc::clone(&retained));
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
    )?;
    api.on(
        "agent_start",
        Rc::new(move |_, context| {
            let (io, slot) = (Rc::clone(&io), Rc::clone(&slot));
            Box::pin(async move {
                let request = slot.armed.borrow_mut().take();
                let Some(request) = request else {
                    return Ok(None);
                };
                *slot.signal.borrow_mut() = context.signal()?;
                let owner = Rc::clone(&slot);
                start(
                    &slot,
                    Box::pin(async move { fetch(io, &owner, request).await }),
                )
                .await?;
                Ok(None)
            })
        }),
    )
}
/// Polls the same future once and retains it only while it is pending.
async fn start(slot: &Slot, mut work: ExtensionFuture<'static, Value>) -> ExtensionResult<Value> {
    let polled = std::future::poll_fn(|cx| Poll::Ready(work.as_mut().poll(cx))).await;
    match polled {
        Poll::Ready(result) => result,
        Poll::Pending => {
            *slot.pending.borrow_mut() = Some(work);
            Ok(json!(true))
        }
    }
}
/// Admits or resumes an operation on the original retained resources.
async fn execute(io: Rc<dyn NativeIo>, slot: &Rc<Slot>, args: &str) -> ExtensionResult<Value> {
    let action: Value = serde_json::from_str(args).map_err(|e| e.to_string())?;
    match action["op"].as_str().unwrap_or_default() {
        "arm" => {
            *slot.armed.borrow_mut() =
                Some(serde_json::from_value(action).map_err(|e| e.to_string())?);
            return Ok(Value::Null);
        }
        "finish" => {
            let work = slot
                .pending
                .borrow_mut()
                .take()
                .ok_or("no pending HTTP operation")?;
            return work.await;
        }
        "aborted" | "drop_response" | "drop_body" | "close" => {
            return observe(slot, action["op"].as_str().unwrap_or_default());
        }
        "read" | "start_read" | "text" | "json" | "aliases" => {
            return consume(slot, action["op"].as_str().unwrap_or_default()).await;
        }
        _ => {}
    }
    let exchange = action["op"] == "exchange";
    let request: Fetch = serde_json::from_value(action).map_err(|e| e.to_string())?;
    let value = fetch(io, slot, request).await?;
    if exchange {
        let response = slot
            .response
            .borrow()
            .clone()
            .ok_or("no retained response")?;
        if !response.ok() {
            return Err(format!("Token exchange failed: {}", response.text().await?));
        }
    }
    Ok(value)
}
/// Releases only selected consumers and observes the original caller signal.
fn observe(slot: &Slot, operation: &str) -> ExtensionResult<Value> {
    match operation {
        "aborted" => {
            return Ok(json!(
                slot.signal
                    .borrow()
                    .as_ref()
                    .is_some_and(AbortSignal::aborted)
            ));
        }
        "drop_response" => {
            slot.response.borrow_mut().take();
        }
        "drop_body" => {
            slot.body.borrow_mut().take();
        }
        "close" => slot
            .body
            .borrow()
            .as_ref()
            .ok_or("no response body")?
            .close()?,
        _ => return Err("unknown response observation".into()),
    }
    Ok(Value::Null)
}
/// Consumes or aliases the same body retained by the fetch command.
async fn consume(slot: &Slot, operation: &str) -> ExtensionResult<Value> {
    if operation == "read" || operation == "start_read" {
        let body = slot.body.borrow().clone().ok_or("no response body")?;
        let work = Box::pin(async move { Ok(json!(body.read().await?)) });
        return if operation == "start_read" {
            start(slot, work).await
        } else {
            work.await
        };
    }
    let response = slot
        .response
        .borrow()
        .clone()
        .ok_or("no retained response")?;
    match operation {
        "text" => Ok(json!(response.text().await?)),
        "json" => response.json().await,
        "aliases" => Ok(json!(Rc::ptr_eq(
            &response.body().ok_or("no response body")?,
            &slot.body.borrow().clone().ok_or("no retained body")?
        ))),
        _ => Err("unknown response operation".into()),
    }
}
/// Fetches with the retained caller signal and keeps independently owned response and body.
async fn fetch(io: Rc<dyn NativeIo>, slot: &Slot, request: Fetch) -> ExtensionResult<Value> {
    let signal = slot.signal.borrow().clone();
    let response = io
        .fetch(Request {
            url: request.url,
            method: request.method,
            headers: request.headers,
            body: request.body,
            signal,
        })
        .await?;
    let value = json!({"status":response.status(), "ok":response.ok(),
        "status_text":response.status_text(), "headers":response.headers(),
        "body":response.body().is_some()});
    *slot.body.borrow_mut() = response.body();
    *slot.response.borrow_mut() = Some(response);
    Ok(value)
}

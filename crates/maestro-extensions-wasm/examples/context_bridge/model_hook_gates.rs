//! Awaited payload and response callbacks park inside their original component invocation.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    Context, ExtensionCommandContext, Model, OnPayload, OnResponse, ProviderStreamOptions,
    SimpleStreamOptions, StreamOptions,
};
use serde::Deserialize;
use serde_json::json;
use std::{cell::RefCell, rc::Rc, sync::Arc};

/// Literal native test operands, without a second model or callback registry.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Request {
    /// Which public entry point to invoke.
    simple: bool,
    /// One isolated edit, with a separate unchanged control.
    action: String,
    /// Actual local protocol peer.
    base_url: String,
}

/// Invoke the native protocol and observe its result independently of event iteration.
pub async fn command(request: &str, context: ExtensionCommandContext) -> Result<String, String> {
    let request: Request = serde_json::from_str(request).map_err(|e| e.to_string())?;
    let model: Model = serde_json::from_value(json!({
        "id":"gated-model","name":"gated","api":"component-gated-http","provider":"custom",
        "baseUrl":request.base_url,"reasoning":false,"input":["text"],
        "cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},
        "contextWindow":1000,"maxTokens":100,"compat":{"maxTokensField":"max_tokens"}
    }))
    .map_err(|e| e.to_string())?;
    let conversation: Context = serde_json::from_value(json!({
        "messages":[{"role":"user","content":"unchanged neighbor","timestamp":1}]
    }))
    .map_err(|e| e.to_string())?;
    let calls = Rc::new(RefCell::new(Vec::new()));
    let common = StreamOptions {
        temperature: Some(0.25),
        max_tokens: Some(17.0),
        api_key: Some("controlled".into()),
        max_retries: Some(0.0),
        signal: context.signal()?,
        on_payload: Some(payload(context.clone(), request.action, calls.clone())),
        on_response: Some(response(context, calls.clone())),
        ..Default::default()
    };
    let stream = if request.simple {
        maestro_extensions_wasm::stream_simple(
            model,
            conversation,
            Some(SimpleStreamOptions {
                common,
                ..Default::default()
            }),
        )?
    } else {
        maestro_extensions_wasm::stream(
            model,
            conversation,
            Some(ProviderStreamOptions {
                common,
                ..Default::default()
            }),
        )?
    };
    let message = stream.result().await?;
    let same_owner = Arc::ptr_eq(&message, &stream.result().await?);
    let mut events = Vec::new();
    while let Some(event) = stream.next().await? {
        events.push(serde_json::to_value(event.as_ref()).map_err(|e| e.to_string())?);
    }
    Ok(json!({"message":message,"sameOwner":same_owner,"events":events,"calls":*calls.borrow(),"eof":stream.next().await?.is_none()}).to_string())
}

/// Make each edit after native readiness has acknowledged that this hook is genuinely pending.
fn payload(
    context: ExtensionCommandContext,
    action: String,
    calls: Rc<RefCell<Vec<&'static str>>>,
) -> OnPayload {
    Rc::new(move |mut input, model| {
        let (context, action, calls) = (context.clone(), action.clone(), calls.clone());
        Box::pin(async move {
            if model.id != "gated-model" || input.payload["temperature"] != json!(0.25) {
                return Err("wrong gated payload operands".into());
            }
            context.wait_for_idle().await?;
            match action.as_str() {
                "edit" => input.payload["temperature"] = json!(0.75),
                "replace" => {
                    input.payload =
                        json!({"model":"replacement","stream":true,"marker":"whole payload"});
                }
                "add" => input.payload["marker"] = json!({"nested":[false,0,"雪"]}),
                "delete" => {
                    input
                        .payload
                        .as_object_mut()
                        .ok_or("expected object")?
                        .shift_remove("max_tokens");
                }
                "abort" => context.reload().await?,
                "keep" => {}
                _ => return Err("unknown gated edit".into()),
            }
            calls.borrow_mut().push("payload");
            Ok(input)
        })
    })
}

/// The first native body poll must await this original response callback.
fn response(context: ExtensionCommandContext, calls: Rc<RefCell<Vec<&'static str>>>) -> OnResponse {
    Rc::new(move |response, model| {
        let (context, calls) = (context.clone(), calls.clone());
        Box::pin(async move {
            if response.status.to_bits() != 200.0_f64.to_bits()
                || response.headers.get("x-controlled").map(String::as_str) != Some("literal")
                || model.id != "gated-model"
            {
                return Err("wrong gated response operands".into());
            }
            context.wait_for_idle().await?;
            calls.borrow_mut().push("response");
            Ok(())
        })
    })
}

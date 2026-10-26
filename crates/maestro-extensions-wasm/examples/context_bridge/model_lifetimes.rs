//! Model streams retain original local hooks and arbitrary caller cancellation after return.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    AbortListener, AbortSignal, AssistantMessageEventStream, CommandOptions, Context, EventStream,
    ExtensionAPI, ExtensionCommandContext, ExtensionFuture, ExtensionResult, Model, OnPayload,
    OnResponse, ProviderStreamOptions, SignalPort, SimpleStreamOptions, StreamOptions,
    Subscription,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc, sync::Arc};

/// Destruction must run outside the callback table and may synchronously allocate another stream.
struct Released {
    /// The actual application action boundary.
    api: ExtensionAPI,
    /// Distinguishable capture from one invocation.
    label: String,
}
impl Drop for Released {
    fn drop(&mut self) {
        let reentry = EventStream::new(|(): &()| false, |()| ());
        let label = if reentry.is_ok() {
            &self.label
        } else {
            "destructor-reentry-failed"
        };
        let _ = self.api.set_session_name(label);
    }
}
/// An arbitrary author signal, not a host-owned cancellation endpoint.
struct Relay {
    /// Original caller flag received by this invocation only.
    inner: AbortSignal,
    /// Capture released with this invocation's final signal binding.
    release: Released,
}
impl SignalPort for Relay {
    fn aborted(&self) -> bool {
        self.inner.aborted()
    }
    fn subscribe(&self, listener: AbortListener) -> ExtensionResult<Subscription> {
        let _ = &self.release;
        self.inner.subscribe(listener)
    }
}
/// Only fixture operands survive between independent command handlers.
struct Kept {
    /// Independent aliases of the native returned buffer.
    streams: Vec<AssistantMessageEventStream>,
    /// Started observations that can be dropped early without stopping the producer.
    observers: Vec<ExtensionFuture<'static, maestro_extensions_wasm::SharedAssistantMessage>>,
}
thread_local! {
    /// No event queue or result settlement is recreated here.
    static KEPT: RefCell<BTreeMap<String, Kept>> = RefCell::default();
}
/// Owned operands of the initiating command, discarded as soon as invocation returns.
#[derive(Deserialize)]
struct Request {
    /// Select the original raw or simple path.
    simple: bool,
    /// Distinguishable overlapping invocation identity.
    id: String,
}
/// Return immediately, retaining only the stream and separately registered observer callback.
pub fn start(
    api: &ExtensionAPI,
    context: &ExtensionCommandContext,
    data: &str,
) -> ExtensionResult<String> {
    let request: Request = serde_json::from_str(data).map_err(|e| e.to_string())?;
    let model: Model = serde_json::from_value(json!({
        "id":request.id,"name":"retained","api":"component-retained-model","provider":"custom",
        "baseUrl":"https://example.invalid","reasoning":false,"input":[],
        "cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"contextWindow":1,"maxTokens":1
    }))
    .map_err(|e| e.to_string())?;
    let common = options(api, context, &request.id)?;
    let stream = invoke(model, common, request.simple)?;
    let mut observers = Vec::new();
    for _ in 0..3 {
        let mut observer = stream.result();
        super::streams::reads::pending(observer.as_mut())?;
        observers.push(observer);
    }
    KEPT.with_borrow_mut(|kept| {
        kept.insert(
            request.id,
            Kept {
                streams: vec![stream.clone(), stream],
                observers,
            },
        )
    });
    api.register_command(
        "model-retained-observe",
        CommandOptions {
            description: None,
            get_argument_completions: None,
            handler: Rc::new(|action, _| Box::pin(async move { Err(observe(&action).await?) })),
        },
    )?;
    Ok("created".into())
}
/// Forward owned operands through the original raw or simple facade.
fn invoke(
    model: Model,
    common: StreamOptions,
    simple: bool,
) -> ExtensionResult<AssistantMessageEventStream> {
    let conversation = Context {
        system_prompt: None,
        messages: Vec::new(),
        tools: None,
    };
    if simple {
        maestro_extensions_wasm::stream_simple(
            model,
            conversation,
            Some(SimpleStreamOptions {
                common,
                ..Default::default()
            }),
        )
    } else {
        maestro_extensions_wasm::stream(
            model,
            conversation,
            Some(ProviderStreamOptions {
                common,
                ..Default::default()
            }),
        )
    }
}
/// Every callback capture has its own observable final release, separate from the signal.
fn options(
    api: &ExtensionAPI,
    context: &ExtensionCommandContext,
    id: &str,
) -> ExtensionResult<StreamOptions> {
    Ok(StreamOptions {
        signal: Some(AbortSignal::new(Rc::new(Relay {
            inner: context.signal()?.ok_or("missing original signal")?,
            release: Released {
                api: api.clone(),
                label: format!("{id}:signal-drop"),
            },
        }))),
        on_payload: Some(payload(api, id)),
        on_response: Some(response(api, id)),
        ..Default::default()
    })
}
/// Retain the payload closure independently of the initiating command.
fn payload(api: &ExtensionAPI, id: &str) -> OnPayload {
    let payload_capture = Released {
        api: api.clone(),
        label: format!("{id}:payload-drop"),
    };
    let payload_id = id.to_owned();
    Rc::new(move |mut input, model| {
        let _ = &payload_capture;
        let result = if model.id == payload_id && input.payload == json!({"original":true}) {
            payload_capture
                .api
                .set_session_name(&format!("{payload_id}:payload"))
        } else {
            Err("wrong retained payload operands".into())
        };
        Box::pin(async move {
            result?;
            input.payload = json!({"selected":model.id});
            Ok(input)
        })
    })
}

/// Retain the response closure independently of the initiating command.
fn response(api: &ExtensionAPI, id: &str) -> OnResponse {
    let response_capture = Released {
        api: api.clone(),
        label: format!("{id}:response-drop"),
    };
    let response_id = id.to_owned();
    Rc::new(move |response, model| {
        let _ = &response_capture;
        let result = if model.id == response_id
            && response.status.to_bits() == 207.5_f64.to_bits()
            && response.headers.get("x-controlled").map(String::as_str) == Some("literal")
        {
            response_capture
                .api
                .set_session_name(&format!("{response_id}:response"))
        } else {
            Err("wrong retained response operands".into())
        };
        Box::pin(async move { result })
    })
}

/// Extract fixture operands before polling or destroying anything that can reenter the host.
async fn observe(action: &str) -> ExtensionResult<String> {
    let (action, id) = action.split_once(' ').ok_or("missing retained action")?;
    if action == "release" {
        let kept = KEPT.with_borrow_mut(|kept| kept.remove(id));
        drop(kept.ok_or("missing retained invocation")?);
        return Ok("null".into());
    }
    let streams = KEPT
        .with_borrow(|kept| kept.get(id).map(|k| k.streams.clone()))
        .ok_or("missing retained streams")?;
    if action == "drop-observers" {
        let observers = KEPT
            .with_borrow_mut(|kept| {
                kept.get_mut(id)
                    .map(|kept| std::mem::take(&mut kept.observers))
            })
            .ok_or("missing observers")?;
        drop(observers);
        return Ok("null".into());
    }
    let message = streams[0].result().await?;
    let same_owner = Arc::ptr_eq(&message, &streams[1].result().await?);
    let mut events: Vec<Value> = Vec::new();
    while let Some(event) = streams[0].next().await? {
        events.push(serde_json::to_value(event.as_ref()).map_err(|e| e.to_string())?);
    }
    Ok(json!({"message":message,"sameOwner":same_owner,"events":events,"eof":streams[1].next().await?.is_none()}).to_string())
}

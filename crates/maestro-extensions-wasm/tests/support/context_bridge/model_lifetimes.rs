//! Model streams retain original local hooks and arbitrary caller cancellation after return.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    AbortListener, AbortSignal, AssistantMessageEventStream, CommandOptions, Context, EventStream,
    ExtensionAPI, ExtensionCommandContext, ExtensionFuture, ExtensionResult, Model, OnPayload,
    OnResponse, ProviderObjects, ProviderStreamOptions, SignalPort, SimpleStreamOptions,
    StreamOptions, Subscription,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc, sync::Arc};

/// Destruction must run outside the callback table and may synchronously allocate another stream.
struct Released {
    /// The original context remains valid without its initiating registration owner.
    context: ExtensionCommandContext,
    /// Distinguishable capture from one invocation.
    label: String,
}
impl Released {
    /// Observe through the retained native context, without reviving a released registration API.
    fn report(&self, label: &str) -> ExtensionResult<()> {
        self.context
            .compact(Some(maestro_extensions_wasm::CompactOptions {
                custom_instructions: Some(label.into()),
                on_complete: None,
                on_error: None,
            }))
    }
}
impl Drop for Released {
    fn drop(&mut self) {
        let reentry = EventStream::new(|(): &()| false, |()| ());
        let label = if reentry.is_ok() {
            &self.label
        } else {
            "destructor-reentry-failed"
        };
        let _ = self.report(label);
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
    /// Retain a nonserializable author value through the actual native invocation.
    #[serde(default)]
    opaque: bool,
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
    let common = options(context, &request.id)?;
    if serde_json::to_value(&common).map_err(|e| e.to_string())? != json!({}) {
        return Err("capabilities appeared in serialized common options".into());
    }
    let objects = request.opaque.then(|| opaque(context, &request.id));
    let stream = invoke(model, common, request.simple, objects)?;
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
    objects: ProviderObjects,
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
        let options = ProviderStreamOptions {
            common,
            objects,
            ..Default::default()
        };
        if let Some(object) = &options.objects {
            let cloned = options.clone();
            if !Rc::ptr_eq(
                object,
                cloned
                    .objects
                    .as_ref()
                    .ok_or("missing cloned opaque owner")?,
            ) || (object
                .downcast_ref::<Opaque>()
                .ok_or("opaque value narrowed")?
                .value)()
                != 42
            {
                return Err("opaque owner or local closure changed".into());
            }
        }
        maestro_extensions_wasm::stream(model, conversation, Some(options))
    }
}
/// Every callback capture has its own observable final release, separate from the signal.
fn options(context: &ExtensionCommandContext, id: &str) -> ExtensionResult<StreamOptions> {
    Ok(StreamOptions {
        signal: Some(AbortSignal::new(Rc::new(Relay {
            inner: context.signal()?.ok_or("missing original signal")?,
            release: Released {
                context: context.clone(),
                label: format!("{id}:signal-drop"),
            },
        }))),
        on_payload: Some(payload(context, id)),
        on_response: Some(response(context, id)),
        ..Default::default()
    })
}
/// Retain the payload closure independently of the initiating command.
fn payload(context: &ExtensionCommandContext, id: &str) -> OnPayload {
    let payload_capture = Released {
        context: context.clone(),
        label: format!("{id}:payload-drop"),
    };
    let payload_id = id.to_owned();
    Rc::new(move |mut input, model| {
        let _ = &payload_capture;
        let result = if model.id == payload_id && input.payload == json!({"original":true}) {
            payload_capture.report(&format!("{payload_id}:payload"))
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
fn response(context: &ExtensionCommandContext, id: &str) -> OnResponse {
    let response_capture = Released {
        context: context.clone(),
        label: format!("{id}:response-drop"),
    };
    let response_id = id.to_owned();
    Rc::new(move |response, model| {
        let _ = &response_capture;
        let result = if model.id == response_id
            && response.status.to_bits() == 207.5_f64.to_bits()
            && response.headers.get("x-controlled").map(String::as_str) == Some("literal")
        {
            response_capture.report(&format!("{response_id}:response"))
        } else {
            Err("wrong retained response operands".into())
        };
        Box::pin(async move { result })
    })
}

/// Extract fixture operands before polling or destroying anything that can reenter the host.
async fn observe(action: &str) -> ExtensionResult<String> {
    let (action, id) = action.split_once(' ').ok_or("missing retained action")?;
    if release(action, id)? {
        return Ok("null".into());
    }
    let streams = KEPT
        .with_borrow(|kept| kept.get(id).map(|k| k.streams.clone()))
        .ok_or("missing retained streams")?;
    if action == "finish" {
        explicit_end(&streams[0])?;
        return Ok("null".into());
    }
    if action != "observe" {
        return Err("unknown retained action".into());
    }
    let message = streams[0].result().await?;
    let same_owner = Arc::ptr_eq(&message, &streams[1].result().await?);
    let mut events: Vec<Value> = Vec::new();
    while let Some(event) = streams[0].next().await? {
        events.push(serde_json::to_value(event.as_ref()).map_err(|e| e.to_string())?);
    }
    Ok(json!({"message":message,"sameOwner":same_owner,"events":events,"eof":streams[1].next().await?.is_none()}).to_string())
}

/// Push and end use the returned native resource, not a new authored stream.
fn explicit_end(stream: &AssistantMessageEventStream) -> ExtensionResult<()> {
    let message = serde_json::from_value(json!({
        "role":"assistant", "api":"controlled", "provider":"custom", "model":"second",
        "content":[{"type":"text", "text":"explicit guest result"}],
        "usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,
            "cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},
        "stopReason":"stop", "timestamp":13.5,
    }))
    .map_err(|e| e.to_string())?;
    let message = Arc::new(std::sync::RwLock::new(message));
    stream.push(maestro_extensions_wasm::AssistantMessageEvent::Start {
        partial: message.clone(),
    })?;
    stream.end(Some(message))
}

/// Move every destructible operand out of fixture storage before releasing it.
fn release(action: &str, id: &str) -> ExtensionResult<bool> {
    if action == "release" {
        let kept = KEPT.with_borrow_mut(|kept| kept.remove(id));
        drop(kept.ok_or("missing retained invocation")?);
        return Ok(true);
    }
    if action == "drop-alias" {
        let alias = KEPT.with_borrow_mut(|kept| {
            kept.get_mut(id).and_then(|kept| {
                (kept.streams.len() > 1)
                    .then(|| kept.streams.pop())
                    .flatten()
            })
        });
        drop(alias.ok_or("missing independent alias")?);
        return Ok(true);
    }
    if action == "drop-observers" {
        let observers = KEPT
            .with_borrow_mut(|kept| {
                kept.get_mut(id)
                    .map(|kept| std::mem::take(&mut kept.observers))
            })
            .ok_or("missing observers")?;
        drop(observers);
        return Ok(true);
    }
    Ok(false)
}

/// Neither serialization nor cloning is required of the author's actual local value.
struct Opaque {
    /// One nonserializable, non-Clone computation retained with the source value.
    value: Box<dyn Fn() -> u32>,
    /// Observe final release outside callback storage, including reentrant stream creation.
    _release: Released,
}
/// Retain an arbitrary author value, never a simulated native client resource.
fn opaque(context: &ExtensionCommandContext, id: &str) -> Rc<dyn std::any::Any> {
    Rc::new(Opaque {
        value: Box::new(|| 42),
        _release: Released {
            context: context.clone(),
            label: format!("{id}:opaque-drop"),
        },
    })
}

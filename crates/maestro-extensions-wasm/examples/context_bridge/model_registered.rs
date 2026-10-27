//! Native callers consume a registered author's original pending stream.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    AssistantMessageEvent as Event, AssistantMessageEventStream, Context, ExtensionAPI, Model,
    PayloadHookInput, ProviderConfig, StreamFunction, StreamOptionsInput,
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};

/// Work is driven by an admitted command, independently of stream observers.
struct Producer {
    /// Original authored stream.
    stream: AssistantMessageEventStream,
    /// Original descriptor.
    model: Model,
    /// Original raw or simple native controls.
    options: StreamOptionsInput,
}
thread_local! {
    /// A single controlled producer, not a guest stream buffer.
    static PRODUCER: RefCell<Option<Producer>> = const { RefCell::new(None) };
}
/// Set up immediately and leave the returned stream genuinely pending.
fn setup(
    model: Model,
    context: &Context,
    options: Option<StreamOptionsInput>,
) -> Result<AssistantMessageEventStream, String> {
    let options = options.ok_or("missing original options")?;
    let common = options.common();
    if common.temperature.map(f64::to_bits) != Some((-0.0_f64).to_bits())
        || common.timeout_ms != Some(17.5)
        || common.max_retries != Some(2.5)
        || common.signal.is_none()
        || common.on_payload.is_none()
        || common.on_response.is_none()
    {
        return Err("native common options changed".into());
    }
    match &options {
        StreamOptionsInput::Raw(raw) if context.system_prompt.as_deref() == Some("raw") => {
            if raw.extra
                != serde_json::from_value(
                    json!({"reasoning":"raw-low","thinkingBudgets":{"high":19}}),
                )
                .map_err(|e| e.to_string())?
            {
                return Err("raw policies changed".into());
            }
        }
        StreamOptionsInput::Simple(simple)
            if context.system_prompt.as_deref() == Some("simple") =>
        {
            if simple.reasoning != Some(maestro_extensions_wasm::RequestedThinkingLevel::High)
                || simple
                    .thinking_budgets
                    .as_ref()
                    .and_then(|budgets| budgets.high)
                    != Some(31.5)
            {
                return Err("simple policies changed".into());
            }
        }
        _ => return Err("original option variant changed".into()),
    }
    retained(model, options)
}
/// Keep producer work separate from the returned stream and its independent observations.
fn retained(
    model: Model,
    options: StreamOptionsInput,
) -> Result<AssistantMessageEventStream, String> {
    let stream = AssistantMessageEventStream::new()?;
    PRODUCER.with(|slot| {
        slot.replace(Some(Producer {
            stream: stream.clone(),
            model,
            options,
        }))
    });
    Ok(stream)
}
/// Use the actual loading facade or retained catalog registration route.
fn register(
    api: &ExtensionAPI,
    catalog: Option<maestro_extensions_wasm::ModelRegistry>,
    mode: &str,
) -> Result<(), String> {
    let mut config: ProviderConfig = serde_json::from_value(json!({"api":"guest-owned-stream","apiKey":"controlled","baseUrl":"https://example.invalid",
        "models":[{"id":"registered-case","name":"registered","reasoning":false,"input":[],
        "cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"contextWindow":100,"maxTokens":10}]})).map_err(|e|e.to_string())?;
    config.callbacks.stream_simple = Some(match mode {
        "agent" => Rc::new(|model, context, options| agent(model, &context, options))
            as StreamFunction<StreamOptionsInput>,
        "http" => Rc::new(http) as StreamFunction<StreamOptionsInput>,
        _ => Rc::new(|model, context, options| setup(model, &context, options))
            as StreamFunction<StreamOptionsInput>,
    });
    config.callbacks.oauth = mode.ends_with("-oauth").then(|| {
        Rc::new(super::oauth::Provider) as Rc<dyn maestro_extensions_wasm::OAuthProviderInterface>
    });
    if let Some(catalog) = catalog {
        catalog.register_provider("compiled-stream", config)
    } else {
        api.register_provider("compiled-stream", config)
    }
}
/// Consume selected payload data only after both original native hook gates complete.
async fn produce(producer: &Producer) -> Result<Value, String> {
    let common = producer.options.common();
    let model = Arc::new(producer.model.clone());
    let selected = common.on_payload.as_ref().ok_or("missing payload hook")?(
        PayloadHookInput {
            payload: json!({"original":true,"unchanged":"neighbor"}),
            signal: common.signal.clone(),
        },
        model.clone(),
    )
    .await?;
    if selected
        .signal
        .as_ref()
        .is_some_and(maestro_extensions_wasm::AbortSignal::aborted)
    {
        return Err("Request was aborted".into());
    }
    common.on_response.as_ref().ok_or("missing response hook")?(
        maestro_extensions_wasm::ProviderResponse {
            status: 207.5,
            headers: std::collections::BTreeMap::from([("x-native".into(), "literal".into())]),
        },
        model,
    )
    .await?;
    Ok(selected.payload)
}
/// Preserve the guest provider's terminal protocol for hook errors and selected cancellation.
async fn finish(producer: Producer) -> Result<(), String> {
    let result = produce(&producer).await;
    let error = result.as_ref().err().cloned();
    let text = result.unwrap_or(Value::Null).to_string();
    let reason = match error.as_deref() {
        None => "stop",
        Some("Request was aborted") => "aborted",
        Some(_) => "error",
    };
    let message = Arc::new(RwLock::new(serde_json::from_value(json!({"role":"assistant","api":"guest-owned-stream",
        "provider":"compiled-stream","model":"registered-case","content":[{"type":"text","text":text}],
        "usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},
        "stopReason":reason,"errorMessage":error,"timestamp":1})).map_err(|e|e.to_string())?));
    if error.is_some() {
        producer.stream.push(Event::Error {
            reason: if reason == "aborted" {
                maestro_extensions_wasm::ErrorReason::Aborted
            } else {
                maestro_extensions_wasm::ErrorReason::Error
            },
            error: message,
        })?;
    } else {
        producer.stream.push(Event::Start {
            partial: message.clone(),
        })?;
        producer.stream.push(Event::TextDelta {
            content_index: 0,
            delta: "ordered delta".into(),
            partial: message.clone(),
        })?;
        producer.stream.push(Event::Done {
            reason: maestro_extensions_wasm::DoneReason::Stop,
            message,
        })?;
    }
    Ok(())
}
/// Drive setup and production separately so native consumers can observe both pending gates.
pub async fn command(
    api: &ExtensionAPI,
    context: &maestro_extensions_wasm::ExtensionCommandContext,
    request: &str,
) -> Result<String, String> {
    match request {
        "create" | "catalog" | "create-oauth" | "catalog-oauth" | "agent" | "http" => {
            let catalog = request
                .starts_with("catalog")
                .then(|| context.model_registry())
                .transpose()?;
            register(api, catalog, request)?;
            Ok("registered".into())
        }
        "produce" => {
            let producer = PRODUCER
                .with(|slot| slot.borrow_mut().take())
                .ok_or("missing producer")?;
            finish(producer).await?;
            Ok("produced".into())
        }
        _ => Err("unknown registered stream command".into()),
    }
}

/// Native agent requests retain their real conversation and original simple capabilities.
fn agent(
    model: Model,
    context: &Context,
    options: Option<StreamOptionsInput>,
) -> Result<AssistantMessageEventStream, String> {
    if !serde_json::to_string(&context.messages)
        .map_err(|e| e.to_string())?
        .contains("native agent 雪")
    {
        return Err("agent conversation changed".into());
    }
    let Some(options @ StreamOptionsInput::Simple(_)) = options else {
        return Err("agent simple options changed".into());
    };
    retained(model, options)
}

/// Forward the registered guest's original option variant to the bundled native protocol.
fn http(
    mut model: Model,
    context: Context,
    options: Option<StreamOptionsInput>,
) -> Result<AssistantMessageEventStream, String> {
    model.api = "component-gated-http".into();
    match options.ok_or("missing original options")? {
        StreamOptionsInput::Raw(options) => {
            maestro_extensions_wasm::stream::stream(model, context, Some(options))
        }
        StreamOptionsInput::Simple(options) => {
            maestro_extensions_wasm::stream::stream_simple(model, context, Some(options))
        }
    }
}

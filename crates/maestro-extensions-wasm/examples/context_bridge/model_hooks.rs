//! Awaited guest hooks select the values consumed by the registered native provider.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    Context, Model, ProviderStreamOptions, SimpleStreamOptions, StreamOptions,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// Exercise each JSON replacement on both raw and simple entry points.
pub async fn command() -> Result<String, String> {
    let model = model("hook-model")?;
    let context = Context {
        system_prompt: None,
        messages: Vec::new(),
        tools: None,
    };
    let cases = vec![
        None,
        Some(Value::Null),
        Some(json!(false)),
        Some(json!(0)),
        Some(json!("")),
        Some(json!(7.5)),
        Some(json!("replacement 雪")),
        Some(json!({"selected":true})),
        Some(json!([null, false, 0])),
    ];
    let mut output = Vec::new();
    for simple in [false, true] {
        for selected in &cases {
            let calls = Rc::new(RefCell::new(Vec::new()));
            let common = options(selected.clone(), calls.clone());
            let stream = if simple {
                maestro_extensions_wasm::stream_simple(
                    model.clone(),
                    context.clone(),
                    Some(SimpleStreamOptions {
                        common,
                        ..Default::default()
                    }),
                )?
            } else {
                maestro_extensions_wasm::stream(
                    model.clone(),
                    context.clone(),
                    Some(ProviderStreamOptions {
                        common,
                        ..Default::default()
                    }),
                )?
            };
            let message =
                serde_json::to_value(stream.result().await?).map_err(|e| e.to_string())?;
            output.push(json!({"simple":simple,"selected":selected,"message":message,"calls":calls.borrow().clone(),"event":serde_json::to_value(stream.next().await?.ok_or("missing terminal")?.as_ref()).map_err(|e|e.to_string())?,"eof":stream.next().await?.is_none()}));
        }
    }
    Ok(Value::Array(output).to_string())
}
/// Keep local callback captures and inspect the actual native phase operands.
fn options(selected: Option<Value>, calls: Rc<RefCell<Vec<Value>>>) -> StreamOptions {
    let payload_calls = calls.clone();
    StreamOptions {
        on_payload: Some(Rc::new(move |mut input, model| {
            let (selected, calls) = (selected.clone(), payload_calls.clone());
            Box::pin(async move {
                if input.payload != json!({"original":true}) || model.id != "hook-model" {
                    return Err("wrong payload operands".into());
                }
                calls.borrow_mut().push(json!("payload"));
                if let Some(selected) = selected {
                    input.payload = selected;
                }
                Ok(input)
            })
        })),
        on_response: Some(Rc::new(move |response, model| {
            let calls = calls.clone();
            Box::pin(async move {
                if response.status.to_bits() != 207.5_f64.to_bits()
                    || response.headers.get("x-controlled").map(String::as_str) != Some("literal")
                    || model.id != "hook-model"
                {
                    return Err("wrong response operands".into());
                }
                calls.borrow_mut().push(json!("response"));
                Ok(())
            })
        })),
        ..Default::default()
    }
}

/// The same controlled registered protocol accepts distinguishable descriptors.
fn model(id: &str) -> Result<Model, String> {
    serde_json::from_value(json!({"id":id,"name":"hooks","api":if id=="signal-model" {"component-hooks-signals"} else {"component-hooks"},"provider":"custom","baseUrl":"https://example.invalid","reasoning":false,"input":[],"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"contextWindow":1,"maxTokens":1})).map_err(|e|e.to_string())
}

/// Preserve, replace or remove the caller's arbitrary preaborted signal in the awaited hook.
pub async fn signals() -> Result<String, String> {
    let mut output = Vec::new();
    for simple in [false, true] {
        for action in ["keep", "replace", "remove"] {
            let common = signal_options(action);
            let context = Context {
                system_prompt: None,
                messages: Vec::new(),
                tools: None,
            };
            let stream = if simple {
                maestro_extensions_wasm::stream_simple(
                    model("signal-model")?,
                    context,
                    Some(SimpleStreamOptions {
                        common,
                        ..Default::default()
                    }),
                )?
            } else {
                maestro_extensions_wasm::stream(
                    model("signal-model")?,
                    context,
                    Some(ProviderStreamOptions {
                        common,
                        ..Default::default()
                    }),
                )?
            };
            output.push(json!({"simple":simple,"action":action,"message":serde_json::to_value(stream.result().await?).map_err(|e|e.to_string())?}));
        }
    }
    Ok(Value::Array(output).to_string())
}

/// Each payload selector returns an independently chosen cancellation capability.
fn signal_options(action: &'static str) -> StreamOptions {
    StreamOptions {
        signal: Some(super::exec::preaborted()),
        on_payload: Some(Rc::new(move |mut input, _| {
            Box::pin(async move {
                match action {
                    "replace" => input.signal = Some(super::exec::preaborted()),
                    "remove" => input.signal = None,
                    _ => {}
                }
                Ok(input)
            })
        })),
        on_response: Some(Rc::new(|_, _| Box::pin(async { Ok(()) }))),
        ..Default::default()
    }
}

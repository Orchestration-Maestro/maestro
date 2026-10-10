//! Shared author operations for the component and controlled host.
use maestro_extensions_wasm::{ExtensionAPI, ExtensionResult};
use serde::Deserialize;
use serde_json::{Value, json};
use std::rc::Rc;

/// One literal emission requested by the test driver.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    /// Literal channel.
    channel: String,
    /// Actual payload.
    input: Value,
}
/// Registers, invokes and explicitly removes a listener through author facades.
pub async fn run(api: &ExtensionAPI, args: &str) -> ExtensionResult<()> {
    let request: Request = serde_json::from_str(args).map_err(|e| e.to_string())?;
    let bus = api.events();
    let nested_log = api.clone();
    let nested = bus.on(
        "received",
        Rc::new(move |data, _| {
            nested_log.append_entry("bus-nested", Some(data))?;
            Ok(None)
        }),
    )?;
    let log = api.clone();
    let subscription = bus.on(
        &request.channel,
        Rc::new(move |data, emitter| {
            let fail = data == json!("prefix-error");
            log.append_entry("bus-prefix", Some(data.clone()))?;
            emitter.emit("received", data)?;
            if fail {
                return Err("component prefix failed Ω".into());
            }
            let log = log.clone();
            Ok(Some(Box::pin(async move {
                log.append_entry("bus-tail", Some(json!("done")))
            })))
        }),
    )?;
    bus.emit(&format!("{}::decoy", request.channel), json!("decoy"))
        .await?;
    bus.emit(&request.channel, request.input).await?;
    subscription.unsubscribe()?;
    nested.unsubscribe()
}

/// Keeps listeners used to witness preparation's same-instance nested emissions.
pub fn register_preparation(api: &ExtensionAPI) -> ExtensionResult<()> {
    let nested_log = api.clone();
    api.events().on(
        "prepare-nested",
        Rc::new(move |data, _| {
            nested_log.append_entry("prepare-nested", Some(data))?;
            Ok(None)
        }),
    )?;
    let log = api.clone();
    api.events().on(
        "prepared-event",
        Rc::new(move |data, emitter| {
            log.append_entry("prepare-prefix", Some(data.clone()))?;
            emitter.emit("prepare-nested", data)?;
            Ok(None)
        }),
    )?;
    Ok(())
}

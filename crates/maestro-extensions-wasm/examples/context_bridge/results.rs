//! Authored preparation and compaction callbacks whose results native witnesses inspect.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    CallbackEmitter, CompactOptions, CompactionComplete, CompactionError, ExtensionAPI,
    ExtensionCommandContext, ExtensionResult,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

thread_local! {
    /// How this instance's preparation answers, selected by the native caller.
    static MODE: RefCell<String> = RefCell::default();
}

/// Selects how later preparations answer.
pub(super) fn set_mode(mode: String) {
    MODE.set(mode);
}

/// Traces one step through a callback emitter.
fn trace(api: &ExtensionAPI, emitter: &CallbackEmitter<'_>, step: &str) -> ExtensionResult<()> {
    let label = super::ordering::label(api)?;
    emitter.emit(&format!("trace:{label}"), json!(format!("{label}:{step}")))
}

/// Registers tool `echo`, with or without argument preparation.
pub(super) fn echo(api: &ExtensionAPI, prepare: bool) -> ExtensionResult<()> {
    let (prepare_api, execute_api) = (api.clone(), api.clone());
    let prepare: Option<maestro_extensions_wasm::PrepareArguments> = prepare.then(|| {
        Rc::new(move |args: Value, emitter: CallbackEmitter<'_>| {
            trace(&prepare_api, &emitter, "prepare")?;
            prepared(&prepare_api, args, &emitter)
        }) as _
    });
    api.register_tool(maestro_extensions_wasm::ToolDefinition {
        metadata: serde_json::from_value(json!({"name":"echo", "label":"Echo", "description":"Echoes its arguments", "parameters":{"type":"object"}})).map_err(|error| error.to_string())?,
        prepare_arguments: prepare,
        execute: Rc::new(move |_, params, _, _, _| {
            let api = execute_api.clone();
            Box::pin(async move {
                let label = super::ordering::label(&api)?;
                api.events().emit(&format!("payload:{label}"), params).await?;
                serde_json::from_value(json!({"content":[],"details":null})).map_err(|error| error.to_string())
            })
        }),
    })
}

/// Answers one preparation under the selected mode.
fn prepared(
    api: &ExtensionAPI,
    mut args: Value,
    emitter: &CallbackEmitter<'_>,
) -> ExtensionResult<Value> {
    let mode = MODE.with_borrow(Clone::clone);
    match mode.as_str() {
        "same" => {}
        "replace" => return Ok(json!({"prepared": args})),
        "throw" => return Err("prepare failed: \u{3a9}\nline".into()),
        "mutate" => args["kept"] = json!("new"),
        "add" => args["added"] = json!("new"),
        "delete" => {
            args.as_object_mut()
                .ok_or("arguments are not an object")?
                .shift_remove("deleted");
        }
        "drain" => {
            emitter.emit("prepare-foreign", Value::Null)?;
            trace(api, emitter, "prepare:end")?;
            return Ok(json!({"prepared": args}));
        }
        mode => return Err(format!("unknown preparation mode {mode}")),
    }
    Ok(args)
}

/// Requests compaction with absent, empty or complete options, then traces the return.
pub(super) async fn compact(
    api: &ExtensionAPI,
    context: &ExtensionCommandContext,
    options: &str,
) -> ExtensionResult<()> {
    let options = match options {
        "absent" => None,
        "empty" => Some(CompactOptions::default()),
        _ => Some(CompactOptions {
            custom_instructions: Some("keep".into()),
            on_complete: Some(completed(api)),
            on_error: Some(failed(api)),
        }),
    };
    context.compact(options)?;
    let label = super::ordering::label(api)?;
    api.events()
        .emit(&format!("trace:{label}"), json!("request:return"))
        .await
}

/// Forwards the finished compaction, then emits to other instances.
fn completed(api: &ExtensionAPI) -> CompactionComplete {
    let api = api.clone();
    Rc::new(move |result, emitter| {
        trace(&api, &emitter, "complete")?;
        let label = super::ordering::label(&api)?;
        let result = serde_json::to_value(result).map_err(|error| error.to_string())?;
        emitter.emit(&format!("payload:{label}"), result)?;
        emitter.emit("compaction-foreign", Value::Null)?;
        trace(&api, &emitter, "complete:end")
    })
}

/// Traces the exact compaction failure, then emits to other instances.
fn failed(api: &ExtensionAPI) -> CompactionError {
    let api = api.clone();
    Rc::new(move |error, emitter| {
        trace(&api, &emitter, &format!("error:{error}"))?;
        emitter.emit("compaction-foreign", Value::Null)?;
        trace(&api, &emitter, "error:end")
    })
}

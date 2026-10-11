//! Authored shell operations shared by native and compiled tests.
use maestro_extensions_wasm::{
    AbortSignal, BashExecOptions, BashExecResult, BashOnData, BashOperations, BashResult,
    ExtensionAPI, ExtensionCommandContext, ExtensionEvent, ExtensionEventResult, ExtensionResult,
    Presence, UserBashEventResult,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};
/// Independently held aliases controlled through the capture command.
#[derive(Default)]
struct Kept {
    /// Author-held operation.
    operation: Option<BashOperations>,
    /// Output retained beyond execution.
    output: Option<BashOnData>,
    /// Signal retained beyond execution.
    signal: Option<AbortSignal>,
    /// Mutable author configuration.
    config: Value,
}
/// Routes registration to the shell fixture.
pub fn dispatch(
    api: &ExtensionAPI,
    args: &str,
    ctx: &ExtensionCommandContext,
) -> Option<ExtensionResult<()>> {
    args.strip_prefix("shell-register ")
        .map(|text| register(api, text, ctx))
}
/// Registers an event and a control handler sharing a retained operation slot.
fn register(api: &ExtensionAPI, text: &str, ctx: &ExtensionCommandContext) -> ExtensionResult<()> {
    let config: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let kept = Rc::new(RefCell::new(Kept {
        config,
        ..Kept::default()
    }));
    let operations = operations(api, &kept, ctx.clone());
    kept.borrow_mut().operation = Some(operations.clone());
    let control_kept = Rc::clone(&kept);
    api.on(
        "shell-control",
        Rc::new(move |event, _| {
            let kept = Rc::clone(&control_kept);
            Box::pin(async move { control(&kept, event) })
        }),
    )?;
    let handler_guard = Guard {
        api: api.clone(),
        label: "shell-handler",
        reenter: kept.borrow().config["reenter"].as_bool().unwrap_or(false),
    };
    let event_kept = kept;
    api.on(
        "user_bash",
        Rc::new(move |event, _| {
            let _ = &handler_guard;
            let kept = Rc::clone(&event_kept);
            let ops = operations.clone();
            Box::pin(async move { answer(&kept, event, ops) })
        }),
    )
}
/// Creates one closure with an independently observable final owner.
fn operations(
    api: &ExtensionAPI,
    kept: &Rc<RefCell<Kept>>,
    ctx: ExtensionCommandContext,
) -> BashOperations {
    let guard = Guard {
        api: api.clone(),
        label: "shell-operation",
        reenter: kept.borrow().config["reenter"].as_bool().unwrap_or(false),
    };
    let api = api.clone();
    let owner = Rc::clone(kept);
    BashOperations {
        exec: Rc::new(move |command, cwd, options| {
            let _ = &guard;
            let kept = Rc::clone(&owner);
            let api = api.clone();
            let ctx = ctx.clone();
            Box::pin(async move { execute(&api, &kept, &ctx, (command, cwd), options).await })
        }),
    }
}
/// Returns the configured independent answer fields.
fn answer(
    kept: &Rc<RefCell<Kept>>,
    event: &mut ExtensionEvent,
    ops: BashOperations,
) -> ExtensionResult<Option<ExtensionEventResult>> {
    let config = kept.borrow().config.clone();
    if let ExtensionEvent::UserBash(event) = event {
        event.command = "edited\0雪".into();
    }
    if let Some(error) = config["handlerError"].as_str() {
        return Err(error.into());
    }
    if config["empty"].is_string() {
        return Ok(None);
    }
    let result = match config["result"].as_str().unwrap_or("missing") {
        "missing" => Presence::Missing,
        "null" => Presence::Null,
        kind => Presence::Present(BashResult {
            output: "replaced\0雪".into(),
            exit_code: Presence::Present(if kind == "nonfinite" {
                match config["number"].as_str() {
                    Some("nan") => f64::NAN,
                    Some("negative-infinity") => f64::NEG_INFINITY,
                    _ => f64::INFINITY,
                }
            } else {
                7.0
            }),
            cancelled: false,
            truncated: true,
            full_output_path: Presence::Present(" x/../out ".into()),
        }),
    };
    Ok(Some(ExtensionEventResult::UserBash(UserBashEventResult {
        operations: config["operations"]
            .as_bool()
            .unwrap_or(true)
            .then_some(ops),
        result,
    })))
}
/// Observes literal execution operands, then streams the configured chunks.
async fn execute(
    api: &ExtensionAPI,
    kept: &Rc<RefCell<Kept>>,
    ctx: &ExtensionCommandContext,
    operands: (String, String),
    options: BashExecOptions,
) -> ExtensionResult<BashExecResult> {
    let config = kept.borrow().config.clone();
    api.append_entry("shell-operands", Some(json!({"command":operands.0,"cwd":operands.1,"timeout":options.timeout.map(|t| t.to_bits().to_string()),"env":options.env.as_ref().map(|env| env.iter().collect::<Vec<_>>()),"initialSignal":options.signal.as_ref().map(AbortSignal::aborted)})))?;
    if config["retain"].as_bool().unwrap_or(false) {
        let mut slot = kept.borrow_mut();
        slot.output = Some(options.on_data.clone());
        slot.signal = options.signal.clone();
    }
    let chunks: Vec<Vec<u8>> =
        serde_json::from_value(config["chunks"].clone()).map_err(|e| e.to_string())?;
    for (index, chunk) in chunks.iter().enumerate() {
        let sent = (options.on_data)(chunk);
        if !config["catch"].as_bool().unwrap_or(false) {
            sent?;
        }
        if index == 0 {
            if let Some(error) = config["error"].as_str() {
                return Err(error.into());
            }
            if config["wait"].as_bool().unwrap_or(false) {
                ctx.wait_for_idle().await?;
            }
        }
    }
    api.append_entry(
        "shell-final",
        Some(json!(options.signal.as_ref().map(AbortSignal::aborted))),
    )?;
    Ok(BashExecResult {
        exit_code: config["exitCode"].as_f64(),
    })
}
/// Uses or removes the exact retained owners without borrowing a slot across callbacks.
fn control(
    kept: &Rc<RefCell<Kept>>,
    event: &mut ExtensionEvent,
) -> ExtensionResult<Option<ExtensionEventResult>> {
    let ExtensionEvent::Input(event) = event else {
        return Err("expected control input".into());
    };
    let value: Value = serde_json::from_str(&event.text).map_err(|e| e.to_string())?;
    match value["action"].as_str().unwrap_or("config") {
        "alias" => {
            return Ok(Some(ExtensionEventResult::UserBash(UserBashEventResult {
                operations: kept.borrow().operation.clone(),
                result: Presence::Missing,
            })));
        }
        "clear-operation" => {
            let old = kept.borrow_mut().operation.take();
            drop(old);
        }
        "clear-resources" => {
            let (output, signal) = {
                let mut slot = kept.borrow_mut();
                (slot.output.take(), slot.signal.take())
            };
            drop(output);
            drop(signal);
        }
        "output" => {
            let output = kept.borrow().output.clone().ok_or("output absent")?;
            output(&[42, 0, 255])?;
        }
        "signal" => {
            let signal = kept.borrow().signal.clone().ok_or("signal absent")?;
            event.text = signal.aborted().to_string();
        }
        _ => {
            kept.borrow_mut().config = value;
        }
    }
    Ok(None)
}

/// Reports final capture destruction and optionally registers a reentrant callback.
struct Guard {
    /// Host handle for public observations.
    api: ExtensionAPI,
    /// Distinguishable closure label.
    label: &'static str,
    /// Whether teardown registers another callback.
    reenter: bool,
}
impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.api.append_entry("released", Some(json!(self.label)));
        if self.reenter {
            let _ = self.api.on("shell-reentered", reentrant_handler());
        }
    }
}

/// Creates the reentrant callback outside its destructor's control flow.
fn reentrant_handler() -> maestro_extensions_wasm::ExtensionHandler {
    Rc::new(|event, _| {
        Box::pin(async move {
            if let ExtensionEvent::Input(input) = event {
                input.text = "reentered".into();
            }
            Ok(None)
        })
    })
}

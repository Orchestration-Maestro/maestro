//! Shared author fixture for tool registration and invocation.
use maestro_extensions_wasm::{AbortSignal, AgentToolUpdateCallback, ExtensionCommandContext};
use std::cell::RefCell;
use std::rc::Rc;

use maestro_extensions_wasm::{
    AgentToolResult, ExtensionAPI, ExtensionResult, ToolDefinition, ToolMetadata,
};
use serde_json::{Value, json};

/// Registers a tool whose supplied configuration controls the callback observations.
pub fn register(
    api: &ExtensionAPI,
    config: &str,
    captured: Rc<RefCell<Option<ExtensionCommandContext>>>,
) -> ExtensionResult<()> {
    let config: Value = serde_json::from_str(config).map_err(|e| e.to_string())?;
    let metadata: ToolMetadata =
        serde_json::from_value(config["metadata"].clone()).map_err(|e| e.to_string())?;
    let kept: Rc<RefCell<Kept>> = Rc::default();
    let prepare_arguments = preparation(api, config.clone(), Rc::clone(&kept));
    let execute = execution(api, config, captured, kept);
    api.register_tool(ToolDefinition {
        metadata,
        prepare_arguments,
        execute,
    })
}

/// Synchronous authored preparation with optional retained-resource controls.
fn preparation(
    api: &ExtensionAPI,
    config: Value,
    kept: Rc<RefCell<Kept>>,
) -> Option<maestro_extensions_wasm::PrepareArguments> {
    let prepare_api = api.clone();
    let prepare_config = config;
    let prepare_kept = kept;
    prepare_config["prepare"]
        .as_bool()
        .unwrap_or(true)
        .then(|| {
            let guard = super::Released::new(api, "tool prepare");
            Rc::new(move |mut args: Value| {
                let _ = &guard;
                if let Some(control) = args["control"].as_str() {
                    return control_kept(&prepare_kept, control);
                }
                if let Some(error) = prepare_config["prepareError"].as_str() {
                    return Err(error.into());
                }
                if args.is_object() {
                    args["mark"] = json!("after");
                }
                prepare_api.append_entry("prepared", Some(args.clone()))?;
                Ok(args)
            }) as maestro_extensions_wasm::PrepareArguments
        })
}

/// Authored asynchronous execution with explicitly retained handles and a named gate.
fn execution(
    api: &ExtensionAPI,
    config: Value,
    captured: Rc<RefCell<Option<ExtensionCommandContext>>>,
    kept: Rc<RefCell<Kept>>,
) -> maestro_extensions_wasm::ToolExecute {
    let execute_api = api.clone();
    let guard = super::Released::new(api, "tool execute");
    Rc::new(move |id, args, signal, update, ctx| {
        let _ = &guard;
        let api = execute_api.clone();
        let captured = Rc::clone(&captured);
        let kept = Rc::clone(&kept);
        let config = config.clone();
        Box::pin(async move {
            api.append_entry(
                "execution",
                Some(json!({
                    "id":id,"args":args,"cwd":ctx.cwd()?,
                    "signal":signal.as_ref().map(maestro_extensions_wasm::AbortSignal::aborted),
                    "update":update.is_some(),
                })),
            )?;
            if let Some(error) = config["executeError"].as_str() {
                return Err(error.into());
            }
            let result: AgentToolResult =
                serde_json::from_value(config["result"].clone()).map_err(|e| e.to_string())?;
            if let Some(update) = &update {
                update(result.clone())?;
            }
            if config["retain"] == true {
                let retained = Kept {
                    signals: signal.iter().flat_map(|s| [s.clone(), s.clone()]).collect(),
                    updates: update.iter().flat_map(|u| [u.clone(), u.clone()]).collect(),
                };
                let old = kept.replace(retained);
                drop(old);
            }
            if config["held"] == true {
                let command = captured.borrow().clone().ok_or("no captured command")?;
                command.wait_for_idle().await?;
                api.append_entry(
                    "after-gate",
                    Some(json!(signal.as_ref().is_some_and(AbortSignal::aborted))),
                )?;
            }
            Ok(result)
        })
    })
}

/// Changes the selected argument and inserts an ordered extra property.
pub fn mark(input: &mut maestro_extensions_wasm::ToolInput, name: &str) {
    let mut value: Value = input.decode().unwrap();
    if let Some(object) = value.as_object_mut() {
        let field = match name {
            "bash" => "command",
            "write" => "content",
            "grep" | "find" => "pattern",
            _ => "path",
        };
        object.insert(field.into(), json!("changed"));
        object.insert("mark".into(), json!("changed"));
    } else {
        value = json!("changed");
    }
    *input = maestro_extensions_wasm::ToolInput::from_value(&value).unwrap();
}

/// Inserts only the extra property observed by the source callback corpus.
pub fn mark_extra(input: &mut maestro_extensions_wasm::ToolInput) {
    let mut value: Value = input.decode().unwrap();
    if let Some(object) = value.as_object_mut() {
        object.insert("mark".into(), json!("changed"));
        *input = maestro_extensions_wasm::ToolInput::from_value(&value).unwrap();
    }
}

/// Two aliases of each retained resource, owned by the callbacks until explicitly removed.
#[derive(Default)]
struct Kept {
    /// Aliased cancellation resources.
    signals: Vec<AbortSignal>,
    /// Aliased progress resources.
    updates: Vec<AgentToolUpdateCallback>,
}

/// Observes or releases retained handles without calling through a borrowed collection.
fn control_kept(kept: &RefCell<Kept>, control: &str) -> ExtensionResult<Value> {
    let mut owned = kept.take();
    if control == "drop-one" {
        drop(owned.signals.pop());
        drop(owned.updates.pop());
    }
    if control == "drop-all" {
        return Ok(Value::Null);
    }
    let aborted = owned.signals.first().is_some_and(AbortSignal::aborted);
    if control == "report"
        && let Some(update) = owned.updates.first()
    {
        update(AgentToolResult {
            content: vec![],
            details: maestro_extensions_wasm::Presence::Missing,
            terminate: maestro_extensions_wasm::Presence::Missing,
        })?;
    }
    let old = kept.replace(owned);
    drop(old);
    Ok(json!(aborted))
}

/// Decodes a typed view, edits a number, and checks finite encoding at construction.
pub fn write_number(
    event: &mut maestro_extensions_wasm::ExtensionEvent,
    field: &str,
    value: f64,
) -> ExtensionResult<()> {
    use maestro_extensions_wasm::*;
    use maestro_extensions_wasm::{ExtensionEvent, Presence, ToolInput};
    macro_rules! edit {
        ($carrier:expr, $ty:ty, $member:ident) => {{
            let carrier = $carrier;
            let mut view: $ty = carrier.decode().map_err(|e| e.to_string())?;
            view.$member = Presence::Present(value);
            *carrier = ToolInput::from_value(&view)?;
        }};
    }
    match event {
        ExtensionEvent::ToolCall(event) => match (event.tool_name.as_str(), field) {
            ("bash", _) => edit!(&mut event.input, BashToolInput, timeout),
            ("read", "offset") => edit!(&mut event.input, ReadToolInput, offset),
            ("read", _) => edit!(&mut event.input, ReadToolInput, limit),
            ("grep", "context") => edit!(&mut event.input, GrepToolInput, context),
            ("grep", _) => edit!(&mut event.input, GrepToolInput, limit),
            ("find", _) => edit!(&mut event.input, FindToolInput, limit),
            ("ls", _) => edit!(&mut event.input, LsToolInput, limit),
            _ => return Err("not numeric input".into()),
        },
        ExtensionEvent::ToolResult(event) => {
            let Presence::Present(details) = &mut event.details else {
                return Err("missing details".into());
            };
            match (event.tool_name.as_str(), field) {
                ("edit", _) => edit!(details, EditToolDetails, first_changed_line),
                ("grep", "matchLimitReached") => {
                    edit!(details, GrepToolDetails, match_limit_reached);
                }
                ("find", "resultLimitReached") => {
                    edit!(details, FindToolDetails, result_limit_reached);
                }
                ("ls", "entryLimitReached") => edit!(details, LsToolDetails, entry_limit_reached),
                _ => write_truncation(details, field, value)?,
            }
        }
        _ => return Err("not a tool event".into()),
    }
    Ok(())
}

/// A view of the accounting shared by tools that supply truncation.
#[derive(serde::Serialize, serde::Deserialize)]
struct Accounting {
    /// Supplied accounting.
    truncation: maestro_extensions_wasm::TruncationResult,
}

/// Assigns each independent accounting field through an explicitly requested view.
fn write_truncation(
    details: &mut maestro_extensions_wasm::ToolDetails,
    field: &str,
    value: f64,
) -> ExtensionResult<()> {
    let mut view: Accounting = details.decode().map_err(|e| e.to_string())?;
    let target = match field {
        "totalLines" => &mut view.truncation.total_lines,
        "totalBytes" => &mut view.truncation.total_bytes,
        "outputLines" => &mut view.truncation.output_lines,
        "outputBytes" => &mut view.truncation.output_bytes,
        "maxLines" => &mut view.truncation.max_lines,
        "maxBytes" => &mut view.truncation.max_bytes,
        _ => return Err("unknown truncation field".into()),
    };
    *target = value;
    *details = maestro_extensions_wasm::ToolDetails::from_value(&view)?;
    Ok(())
}

/// The custom author parses and edits its own string payload; the transport does not.
pub fn mark_custom(input: &mut maestro_extensions_wasm::ToolInput) -> ExtensionResult<()> {
    let text: String = input.decode().map_err(|e| e.to_string())?;
    let mut value: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    value["a"] = json!("changed");
    let text = serde_json::to_string(&value).map_err(|e| e.to_string())?;
    *input = maestro_extensions_wasm::ToolInput::from_value(&text)?;
    Ok(())
}

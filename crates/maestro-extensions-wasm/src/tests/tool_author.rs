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

/// Changes a typed argument and inserts an ordered extra property.
pub fn mark(input: &mut maestro_extensions_wasm::ToolInput) {
    use maestro_extensions_wasm::ToolInput;
    let extra = match input {
        ToolInput::Bash(input) => {
            input.command = "changed".into();
            &mut input.extra
        }
        ToolInput::Read(input) => {
            input.path = "changed".into();
            &mut input.extra
        }
        ToolInput::Edit(input) => {
            input.path = "changed".into();
            &mut input.extra
        }
        ToolInput::Write(input) => {
            input.content = "changed".into();
            &mut input.extra
        }
        ToolInput::Grep(input) => {
            input.pattern = "changed".into();
            &mut input.extra
        }
        ToolInput::Find(input) => {
            input.pattern = "changed".into();
            &mut input.extra
        }
        ToolInput::Ls(input) => {
            input.path = maestro_extensions_wasm::Presence::Present("changed".into());
            &mut input.extra
        }
        ToolInput::Custom(input) => {
            *input = "changed".into();
            return;
        }
    };
    extra.insert("mark".into(), json!("changed"));
}

/// Inserts only the extra property observed by the source callback corpus.
pub fn mark_extra(input: &mut maestro_extensions_wasm::ToolInput) {
    use maestro_extensions_wasm::ToolInput;
    let extra = match input {
        ToolInput::Bash(input) => &mut input.extra,
        ToolInput::Read(input) => &mut input.extra,
        ToolInput::Edit(input) => &mut input.extra,
        ToolInput::Write(input) => &mut input.extra,
        ToolInput::Grep(input) => &mut input.extra,
        ToolInput::Find(input) => &mut input.extra,
        ToolInput::Ls(input) => &mut input.extra,
        ToolInput::Custom(_) => return,
    };
    extra.insert("mark".into(), json!("changed"));
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

/// Writes typed numeric fields without converting the nonfinite values through JSON.
pub fn write_number(
    event: &mut maestro_extensions_wasm::ExtensionEvent,
    field: &str,
    value: f64,
) -> ExtensionResult<()> {
    use maestro_extensions_wasm::{ExtensionEvent, Presence, ToolDetails, ToolInput};
    let optional = match event {
        ExtensionEvent::ToolCall(event) => match &mut event.input {
            ToolInput::Bash(input) => &mut input.timeout,
            ToolInput::Read(input) if field == "offset" => &mut input.offset,
            ToolInput::Read(input) => &mut input.limit,
            ToolInput::Grep(input) if field == "context" => &mut input.context,
            ToolInput::Grep(input) => &mut input.limit,
            ToolInput::Find(input) => &mut input.limit,
            ToolInput::Ls(input) => &mut input.limit,
            _ => return Err("not numeric input".into()),
        },
        ExtensionEvent::ToolResult(event) => {
            let Presence::Present(details) = &mut event.details else {
                return Err("missing details".into());
            };
            match details {
                ToolDetails::Edit(details) => &mut details.first_changed_line,
                ToolDetails::Grep(details) if field == "matchLimitReached" => {
                    &mut details.match_limit_reached
                }
                ToolDetails::Find(details) if field == "resultLimitReached" => {
                    &mut details.result_limit_reached
                }
                ToolDetails::Ls(details) if field == "entryLimitReached" => {
                    &mut details.entry_limit_reached
                }
                _ => return write_truncation(details, field, value),
            }
        }
        _ => return Err("not a tool event".into()),
    };
    *optional = Presence::Present(value);
    Ok(())
}

/// Assigns each independent accounting field in the selected details owner.
fn write_truncation(
    details: &mut maestro_extensions_wasm::ToolDetails,
    field: &str,
    value: f64,
) -> ExtensionResult<()> {
    use maestro_extensions_wasm::{Presence, ToolDetails};
    let truncation = match details {
        ToolDetails::Bash(details) => &mut details.truncation,
        ToolDetails::Read(details) => &mut details.truncation,
        ToolDetails::Grep(details) => &mut details.truncation,
        ToolDetails::Find(details) => &mut details.truncation,
        ToolDetails::Ls(details) => &mut details.truncation,
        _ => return Err("no truncation owner".into()),
    };
    let Presence::Present(truncation) = truncation else {
        return Err("missing truncation".into());
    };
    let target = match field {
        "totalLines" => &mut truncation.total_lines,
        "totalBytes" => &mut truncation.total_bytes,
        "outputLines" => &mut truncation.output_lines,
        "outputBytes" => &mut truncation.output_bytes,
        "maxLines" => &mut truncation.max_lines,
        "maxBytes" => &mut truncation.max_bytes,
        _ => return Err("unknown truncation field".into()),
    };
    *target = value;
    Ok(())
}

/// The custom author parses and edits its own opaque data; the transport does not.
pub fn mark_custom(input: &mut maestro_extensions_wasm::ToolInput) -> ExtensionResult<()> {
    let maestro_extensions_wasm::ToolInput::Custom(input) = input else {
        return Err("not custom".into());
    };
    let mut value: Value = serde_json::from_str(input).map_err(|e| e.to_string())?;
    value["a"] = json!("changed");
    *input = serde_json::to_string(&value).map_err(|e| e.to_string())?;
    Ok(())
}

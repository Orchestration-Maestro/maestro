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
    let config: Value = super::nested_schema_fixture::value(config)?;
    let metadata: ToolMetadata =
        serde_json::from_value(config["metadata"].clone()).map_err(|e| e.to_string())?;
    let metadata = configure_schema(api, &config, metadata)?;
    let kept: Rc<RefCell<Kept>> = Rc::default();
    let prepare_arguments = preparation(api, config.clone(), Rc::clone(&kept));
    let execute = execution(api, config, captured, kept);
    api.register_tool(ToolDefinition {
        render_call: None,
        render_result: None,
        metadata,
        prepare_arguments,
        execute,
    })
}

/// Assemble the authored schema using the existing declaration and executable-node owners.
fn configure_schema(
    api: &ExtensionAPI,
    config: &Value,
    mut metadata: ToolMetadata,
) -> ExtensionResult<ToolMetadata> {
    if let Some(kinds) = config["metadata"].get("schemaKinds") {
        let kinds: std::collections::HashMap<String, maestro_extensions_wasm::SchemaKind> =
            serde_json::from_value(kinds.clone()).map_err(|e| e.to_string())?;
        metadata.parameters = maestro_extensions_wasm::ParameterSchema::authored(
            metadata.parameters.into_json(),
            kinds,
        );
    }
    if let Some(id) = config["corpusCase"].as_str() {
        metadata.parameters = super::nested_schema_fixture::parameters(id)?
            .with_code(std::collections::HashMap::default());
    }
    if config["relay"] == true {
        metadata.parameters = api.get_all_tools()?.remove(0).parameters;
    }
    if config["schemaCode"] == true {
        let code = super::schema_author::object_nodes(&metadata.parameters);
        metadata.parameters = metadata.parameters.with_code(code);
    }
    if let Some(program) = config["schemaProgram"].as_str() {
        let node = if program == "defaults" {
            maestro_extensions_wasm::schema_code::SchemaNode {
                base: Some(maestro_extensions_wasm::schema_code::Base::default()),
                refinements: Vec::new(),
            }
        } else if program == "null-conversion" {
            super::schema_author::null_conversion_node()
        } else if program == "emitting" {
            super::schema_author::emitting_node(api)?
        } else if matches!(program, "deep-read" | "deep-splice") {
            super::schema_author::deep_node(program == "deep-splice")
        } else if program == "operand" {
            super::schema_author::operand_node()
        } else {
            let error = config
                .get("schemaFailure")
                .cloned()
                .map(serde_json::from_value)
                .transpose()
                .map_err(|error| error.to_string())?;
            super::schema_author::node(
                api,
                error.as_ref(),
                config["schemaPhase"].as_str().unwrap_or(""),
            )
        };
        metadata.parameters = metadata
            .parameters
            .with_code(std::collections::HashMap::from([(
                config["schemaPointer"].as_str().unwrap_or("").to_owned(),
                node,
            )]));
    }
    Ok(metadata)
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
            Rc::new(
                move |args: &mut Value, emitter: maestro_extensions_wasm::CallbackEmitter<'_>| {
                    let _ = &guard;
                    if let Some(control) = args["control"].as_str() {
                        return control_kept(&prepare_kept, control);
                    }
                    emit_prepared(&prepare_api, &prepare_config, &emitter)?;
                    if let Some(error) = prepare_config["prepareError"].as_str() {
                        return Err(error.into());
                    }
                    if args.is_object() {
                        args["mark"] = json!("after");
                    }
                    prepare_api.append_entry("prepared", Some(args.clone()))?;
                    Ok(args.clone())
                },
            ) as maestro_extensions_wasm::PrepareArguments
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
pub fn mark(input: &mut maestro_extensions_wasm::ToolInput, name: &str) -> ExtensionResult<()> {
    let mut value: Value = input.decode().map_err(|e| e.to_string())?;
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
    *input = maestro_extensions_wasm::ToolInput::from_value(&value)?;
    Ok(())
}

/// Inserts only the extra property observed by the source callback corpus.
pub fn mark_extra(input: &mut maestro_extensions_wasm::ToolInput) -> ExtensionResult<()> {
    let mut value: Value = input.decode().map_err(|e| e.to_string())?;
    if let Some(object) = value.as_object_mut() {
        object.insert("mark".into(), json!("changed"));
        *input = maestro_extensions_wasm::ToolInput::from_value(&value)?;
    }
    Ok(())
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
            usage: None,
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
    use maestro_extensions_wasm::{
        BashToolInput, EditToolDetails, ExtensionEvent, FindToolDetails, FindToolInput,
        GrepToolDetails, GrepToolInput, LsToolDetails, LsToolInput, Presence, ReadToolInput,
        ToolInput,
    };
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

/// Emits configured preparation data, optionally recording a caught failure.
fn emit_prepared(
    api: &ExtensionAPI,
    config: &Value,
    emitter: &maestro_extensions_wasm::CallbackEmitter<'_>,
) -> ExtensionResult<()> {
    let Some(emissions) = config["emissions"].as_array() else {
        return Ok(());
    };
    for data in emissions {
        if let Err(error) = emitter.emit("prepared-event", data.clone()) {
            if config["catchEmission"].as_bool() != Some(true) {
                return Err(error);
            }
            api.append_entry("caught-emission", Some(json!(error)))?;
        }
    }
    Ok(())
}

//! Synchronous composition of enumerated schema callbacks.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::schema_code::{
    DiagnosticErrorInfo, Refinement, SchemaCallback, SchemaNode,
};
use maestro_extensions_wasm::{ExtensionAPI, ExtensionResult, ParameterSchema, ToolDefinition};
use serde_json::{Value, json};
use std::rc::Rc;

/// Register a concrete node as a tool the native checker can invoke.
fn register(api: &ExtensionAPI, name: &str, node: SchemaNode) -> ExtensionResult<Value> {
    let mut metadata: maestro_extensions_wasm::ToolMetadata = serde_json::from_value(json!({"name":name,"label":"Schema","description":"Synchronous composition","parameters":{}})).map_err(|error| error.to_string())?;
    metadata.parameters = ParameterSchema::<()>::from(json!({}))
        .with_code([(String::new(), node)].into_iter().collect());
    api.register_tool(ToolDefinition {
        metadata,
        prepare_arguments: None,
        execute: Rc::new(|_, _, _, _, _| Box::pin(async { Err("not executed".into()) })),
        render_call: None,
        render_result: None,
    })?;
    Ok(json!(name))
}

/// Inspect every foreign method on a selected view, including an error after an edit.
pub(super) fn inspect(api: &ExtensionAPI) -> ExtensionResult<Value> {
    let node = supplied(api, "schema-executable")?;
    register(
        api,
        "schema-inspected",
        SchemaNode {
            base: None,
            refinements: vec![Refinement {
                check: SchemaCallback::new(move |value, emitter| {
                    let emission = Emission(emitter);
                    let emitter = || maestro_extensions_wasm::CallbackEmitter::new(&emission);
                    let base = node
                        .base
                        .as_ref()
                        .ok_or_else(|| DiagnosticErrorInfo::error("source has no Base"))?;
                    let mut subject = value.member("a/b~");
                    let converted = base.convert.call(&mut subject, emitter())?;
                    let checked = base.check.call(&mut subject, emitter())?;
                    let errors = base.errors.call(&mut subject, emitter())?;
                    let first = node
                        .refinements
                        .first()
                        .ok_or_else(|| DiagnosticErrorInfo::error("first refinement missing"))?;
                    let accepted = first.check.call(&mut subject, emitter())?;
                    let message = first.error.call(&mut subject, emitter())?;
                    let second = node
                        .refinements
                        .get(1)
                        .ok_or_else(|| DiagnosticErrorInfo::error("second refinement missing"))?;
                    let failure = second
                        .check
                        .call(&mut subject, emitter())
                        .err()
                        .ok_or_else(|| DiagnosticErrorInfo::error("expected schema error"))?;
                    value.set(
                        "observed",
                        &json!([
                            converted.is_some(),
                            converted,
                            checked,
                            errors,
                            accepted,
                            message,
                            failure
                        ]),
                    )?;
                    Ok(true)
                }),
                error: SchemaCallback::new(|_, _| Ok("unreached".into())),
            }],
        },
    )
}

/// Lend one callback frame to several consecutive method calls.
struct Emission<'a>(maestro_extensions_wasm::CallbackEmitter<'a>);
impl maestro_extensions_wasm::CallbackEmitterPort for Emission<'_> {
    fn emit(&self, channel: &str, data: Value) -> ExtensionResult<()> {
        self.0.emit(channel, data)
    }
}

/// Keep captures until the last schema identity has been released by the pump.
struct Capture(ExtensionAPI);
impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self.0.set_session_name("schema-capture-dropped");
    }
}

/// Register a synchronous leaf that reports admission before completing its edits.
pub(super) fn leaf(api: &ExtensionAPI) -> ExtensionResult<Value> {
    let capture = Capture(api.clone());
    register(
        api,
        "schema-leaf",
        SchemaNode {
            base: None,
            refinements: vec![Refinement {
                check: SchemaCallback::new(move |value, emitter| {
                    let _ = &capture;
                    value
                        .member("trace")
                        .splice(u32::MAX, 0, &[json!("schema-leaf")])?;
                    value.set("admitted", &json!(true))?;
                    emitter
                        .emit("schema-enter", Value::Null)
                        .map_err(DiagnosticErrorInfo::error)?;
                    value.set("completed", &json!(true))?;
                    Ok(true)
                }),
                error: SchemaCallback::new(|_, _| Ok("unreached".into())),
            }],
        },
    )
}

/// Compose a received callback synchronously and report whether its identity stayed local.
pub(super) fn chain(api: &ExtensionAPI, source: &str, name: &str) -> ExtensionResult<Value> {
    let node = supplied(api, source)?;
    let local = node == supplied(api, source)?;
    let check = node
        .refinements
        .first()
        .ok_or("source has no refinement")?
        .check
        .clone();
    let label = name.to_owned();
    register(
        api,
        name,
        SchemaNode {
            base: None,
            refinements: vec![Refinement {
                check: SchemaCallback::new(move |value, emitter| {
                    value.member("trace").splice(u32::MAX, 0, &[json!(label)])?;
                    check.call(value, emitter)
                }),
                error: SchemaCallback::new(|_, _| Ok("unreached".into())),
            }],
        },
    )?;
    Ok(json!(local))
}

/// Provide every method with the same results as the native control and a textual code.
pub(super) fn methods(api: &ExtensionAPI) -> ExtensionResult<Value> {
    use maestro_extensions_wasm::{DiagnosticCode, schema_code::Base};
    register(
        api,
        "schema-executable",
        SchemaNode {
            base: Some(Base {
                convert: SchemaCallback::new(|value, _| {
                    value.set("native", &json!(true))?;
                    Ok(Some(Value::Null))
                }),
                check: SchemaCallback::new(|value, _| {
                    Ok(value.member("native").read()? == Some(json!(true)))
                }),
                errors: SchemaCallback::new(|_, _| Ok(vec![json!({"message":"native detail"})])),
            }),
            refinements: vec![
                Refinement {
                    check: SchemaCallback::new(|_, _| Ok(false)),
                    error: SchemaCallback::new(|_, _| Ok("native refinement".into())),
                },
                Refinement {
                    check: SchemaCallback::new(|value, _| {
                        value.set("error-edit", &json!(true))?;
                        Err(DiagnosticErrorInfo {
                            name: Some("NativeSchemaError".into()),
                            message: "native failure".into(),
                            stack: Some("native stack".into()),
                            code: Some(DiagnosticCode::Text("GUEST".into())),
                        })
                    }),
                    error: SchemaCallback::new(|_, _| Ok("unreached".into())),
                },
            ],
        },
    )
}

/// Read the requested tool's original root callbacks from the live listing.
fn supplied(api: &ExtensionAPI, source: &str) -> ExtensionResult<SchemaNode> {
    api.get_all_tools()?
        .into_iter()
        .find(|tool| tool.name == source)
        .and_then(|tool| tool.parameters.code().get("").cloned())
        .ok_or_else(|| "source has no code".into())
}

//! Executable schema declarations authored only through the guest SDK.
use maestro_extensions_wasm::schema_code::{
    Base, JsonKind, JsonView, Refinement, SchemaCallback, SchemaNode, SchemaResult,
};
use maestro_extensions_wasm::{DiagnosticErrorInfo, ExtensionAPI};
use serde_json::json;
use std::rc::Rc;

/// Keep the author capture until its last independently retained identity is released.
pub fn node(api: &ExtensionAPI, failure: Option<&DiagnosticErrorInfo>, phase: &str) -> SchemaNode {
    let capture = Rc::new(super::Released::new(api, "schema"));
    SchemaNode {
        base: Some(base(Rc::clone(&capture), failure, phase)),
        refinements: (0..2)
            .map(|index| refinement(Rc::clone(&capture), failure, phase, index))
            .collect(),
    }
}
/// The three separately reached Base methods share only the author's capture.
fn base(capture: Rc<super::Released>, failure: Option<&DiagnosticErrorInfo>, phase: &str) -> Base {
    let conversion_capture = Rc::clone(&capture);
    let conversion_failure = failure.cloned();
    let conversion_phase = phase.to_owned();
    let guard_capture = Rc::clone(&capture);
    let guard_failure = failure.cloned();
    let guard_phase = phase.to_owned();
    let diagnostic_failure = failure.cloned();
    let diagnostic_phase = phase.to_owned();
    Base {
        convert: SchemaCallback::new(move |value, _| {
            let _ = &conversion_capture;
            fail(conversion_failure.as_ref(), &conversion_phase, "convert")?;
            value.set("converted", &json!(true))?;
            Ok(Some(json!({"replacement":true})))
        }),
        check: SchemaCallback::new(move |value, _| {
            let _ = &guard_capture;
            fail(guard_failure.as_ref(), &guard_phase, "check")?;
            value.set("checked", &json!(true))?;
            Ok(false)
        }),
        errors: SchemaCallback::new(move |_, _| {
            let _ = &capture;
            fail(diagnostic_failure.as_ref(), &diagnostic_phase, "errors")?;
            Ok(vec![json!({"keyword":"guard", "message":"author detail"})])
        }),
    }
}
/// Both checks and message factories retain their independent slot and error phase.
fn refinement(
    capture: Rc<super::Released>,
    failure: Option<&DiagnosticErrorInfo>,
    phase: &str,
    index: u32,
) -> Refinement {
    let check_capture = Rc::clone(&capture);
    let check_failure = failure.cloned();
    let check_phase = phase.to_owned();
    let message_failure = failure.cloned();
    let message_phase = phase.to_owned();
    Refinement {
        check: SchemaCallback::new(move |value, _| {
            let _ = &check_capture;
            value.set(&format!("refine{index}"), &json!(true))?;
            fail(
                check_failure.as_ref(),
                &check_phase,
                &format!("refine{index}"),
            )?;
            Ok(index == 0)
        }),
        error: SchemaCallback::new(move |_, _| {
            let _ = &capture;
            fail(
                message_failure.as_ref(),
                &message_phase,
                &format!("message{index}"),
            )?;
            Ok("same text".into())
        }),
    }
}

/// Return the supplied diagnostic unchanged at exactly its selected phase.
fn fail(error: Option<&DiagnosticErrorInfo>, selected: &str, current: &str) -> SchemaResult<()> {
    if selected == current
        && let Some(error) = error
    {
        return Err(error.clone());
    }
    Ok(())
}

/// Exercise source-admitted value access and mutation through the borrowed view.
pub fn operand_node() -> SchemaNode {
    SchemaNode {
        base: None,
        refinements: vec![Refinement {
            check: SchemaCallback::new(|value, _| {
                assert_eq!(value.member("missing").kind()?, None);
                assert_eq!(value.member("null").kind()?, Some(JsonKind::Null));
                assert_eq!(value.member("text").read()?, Some(json!("text")));
                assert_eq!(value.member("number").read()?, Some(json!(3)));
                assert_eq!(value.member("truth").read()?, Some(json!(true)));
                assert!(
                    value
                        .member("zero")
                        .read()?
                        .and_then(|value| value.as_f64())
                        .is_some_and(f64::is_sign_negative)
                );
                assert_eq!(
                    value.member("object").keys()?,
                    ["2", "10", "02", "first", "second"]
                );
                {
                    let mut object = value.member("object");
                    object.delete("first")?;
                    object.set("third", &json!(3))?;
                }
                {
                    let mut array = value.member("array");
                    assert_eq!(
                        array.splice(1, 1, &[json!("inserted"), json!(null)])?,
                        [json!(2)]
                    );
                    array.set("0", &json!(false))?;
                    array.delete("2")?;
                }
                let result = edit_child(value);
                assert!(matches!(result, Err(error) if error.message == "child failure"));
                assert_eq!(
                    value.member("child").member("kept").read()?,
                    Some(json!(true))
                );
                Ok(true)
            }),
            error: SchemaCallback::new(|_, _| Ok(String::new())),
        }],
    }
}
/// A child view remains attached when its author's operation returns an error.
fn edit_child(value: &mut JsonView<'_>) -> SchemaResult<()> {
    let mut child = value.member("child");
    child.set("kept", &json!(true))?;
    Err(DiagnosticErrorInfo::error("child failure"))
}

/// Retain a listener that must run before the schema check's later edit.
pub fn emitting_node(api: &ExtensionAPI) -> Result<SchemaNode, String> {
    let log = api.clone();
    api.events().on(
        "schema-event",
        Rc::new(move |value, _| {
            log.append_entry("schema-listener", Some(value))?;
            Ok(None)
        }),
    )?;
    Ok(SchemaNode {
        base: None,
        refinements: vec![Refinement {
            check: SchemaCallback::new(|value, emitter| {
                value.set("before", &json!(true))?;
                emitter
                    .emit("schema-event", json!({"before":true}))
                    .map_err(DiagnosticErrorInfo::error)?;
                value.set("after", &json!(true))?;
                Ok(true)
            }),
            error: SchemaCallback::new(|_, _| Ok(String::new())),
        }],
    })
}

/// Mark each authored object node without reading or encoding its nested subtree.
pub fn object_nodes(
    parameters: &maestro_extensions_wasm::ParameterSchema<
        maestro_extensions_wasm::schema_code::SchemaCode,
    >,
) -> maestro_extensions_wasm::schema_code::SchemaCode {
    parameters
        .conversions()
        .iter()
        .filter(|(_, kind)| **kind == maestro_extensions_wasm::SchemaKind::Object)
        .map(|(pointer, _)| {
            (
                pointer.clone(),
                SchemaNode {
                    base: None,
                    refinements: vec![Refinement {
                        check: SchemaCallback::new(mark_object),
                        error: SchemaCallback::new(|_, _| Ok("object required".into())),
                    }],
                },
            )
        })
        .collect()
}
/// Object checks preserve their direct native operand and stop before editing other kinds.
fn mark_object(
    value: &mut JsonView<'_>,
    _: maestro_extensions_wasm::CallbackEmitter<'_>,
) -> SchemaResult<bool> {
    if !matches!(value.kind()?, Some(JsonKind::Object)) {
        return Ok(false);
    }
    value.set("seen", &json!(true))?;
    Ok(true)
}

/// Read and roundtrip a nested operand through each JSON resource operation.
pub fn deep_node(splice: bool) -> SchemaNode {
    SchemaNode {
        base: None,
        refinements: vec![Refinement {
            check: SchemaCallback::new(move |value, _| {
                if splice {
                    let removed = value.member("array").splice(0, 1, &[])?;
                    assert_eq!(removed.len(), 1);
                    value.set("written", &removed[0])?;
                    value.member("array").splice(0, 0, &removed)?;
                    return Ok(true);
                }
                let nested = value
                    .member("nested")
                    .read()?
                    .ok_or_else(|| DiagnosticErrorInfo::error("missing nested operand"))?;
                value.set("written", &nested)?;
                Ok(true)
            }),
            error: SchemaCallback::new(|_, _| Ok("nested access failed".into())),
        }],
    }
}

/// Distinguish a null replacement from keeping an existing number.
pub fn null_conversion_node() -> SchemaNode {
    SchemaNode {
        base: Some(Base {
            convert: SchemaCallback::new(|value, _| {
                Ok((value.read()? == Some(json!(""))).then_some(serde_json::Value::Null))
            }),
            check: SchemaCallback::new(|value, _| {
                Ok(matches!(
                    value.kind()?,
                    Some(JsonKind::Null | JsonKind::Number)
                ))
            }),
            ..Base::default()
        }),
        refinements: Vec::new(),
    }
}

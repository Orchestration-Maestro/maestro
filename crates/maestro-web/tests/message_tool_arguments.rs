//! Model object admission through the public browser tool dispatch.
#![cfg(test)]
use crate::tool_renderer_support;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::wasm_bindgen;

use maestro_web::tools::types::{
    ToolParams, ToolRendererBinding, ToolResultMessageView, ToolUiError,
};
use maestro_web::{ToolRenderer, register_tool_renderer, render_tool};
use serde_json::{Map, Value, json};
use std::{cell::Cell, rc::Rc};

use tool_renderer_support::{Renderer, content, message, reset};

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Arguments {
    message: String,
}

#[test]
fn typed_object_arguments_keep_envelope_and_decoder_failures() {
    reset();
    let calls = Rc::new(Cell::new(0));
    let mut result = message(Some(json!({"message": "detail"})));
    register_typed_object(&calls, &result);
    let object: Map<String, Value> =
        serde_json::from_value(json!({"message": "producer value"})).unwrap();
    let prepared = render_tool(
        "object-admission",
        Some(ToolParams::object(&object)),
        Some(&result),
        true,
    );
    assert!(prepared.is_ok(), "model object missed typed callback");
    assert_eq!(calls.get(), 1);
    result.details = None;
    assert!(
        render_tool(
            "object-admission",
            Some(ToolParams::object(&object)),
            Some(&result),
            true
        )
        .is_ok()
    );
    assert_eq!(calls.get(), 2);
    for invalid in [
        json!({}),
        json!({"message": false}),
        json!({"message": "producer value", "extra": true}),
    ] {
        let invalid: Map<String, Value> = serde_json::from_value(invalid).unwrap();
        assert!(
            render_tool(
                "object-admission",
                Some(ToolParams::object(&invalid)),
                Some(&result),
                true
            )
            .is_err()
        );
    }
    for details in [
        Value::Null,
        json!({"message": false}),
        json!({"message": "detail", "extra": true}),
    ] {
        let invalid = message(Some(details));
        assert!(
            render_tool(
                "object-admission",
                Some(ToolParams::object(&object)),
                Some(&invalid),
                true
            )
            .is_err()
        );
    }
    assert_eq!(calls.get(), 2, "decoder failures invoked the callback");
}

#[test]
fn raw_object_arguments_keep_identity_and_lazy_formatting() {
    reset();
    let object: Map<String, Value> =
        serde_json::from_value(json!({"message": "producer value", "nested": [null, false, 0]}))
            .unwrap();
    let address = std::ptr::from_ref(&object) as usize;
    let calls = Rc::new(Cell::new(0));
    let count = calls.clone();
    let renderer: Rc<dyn ToolRenderer<Map<String, Value>>> = Rc::new(Renderer(Rc::new(
        move |params: Option<&Map<String, Value>>,
              _: Option<ToolResultMessageView<'_>>,
              streaming| {
            let params = params.unwrap();
            assert_eq!(std::ptr::from_ref(params) as usize, address);
            assert_eq!(params["message"], "producer value");
            assert_eq!(params["nested"], json!([null, false, 0]));
            assert!(streaming);
            count.set(count.get() + 1);
            Ok(content("raw object", true))
        },
    )));
    let binding = ToolRendererBinding::from_raw(renderer.clone());
    assert!(Rc::ptr_eq(
        &binding.renderer::<Map<String, Value>, Value>().unwrap(),
        &renderer
    ));
    register_tool_renderer("raw-object", binding);
    let format =
        |_: &Map<String, Value>| Err(ToolUiError::Presentation("formatter reached".into()));
    assert!(
        render_tool::<_, Value>(
            "raw-object",
            Some(ToolParams::typed(&object, format)),
            None,
            true
        )
        .unwrap()
        .is_custom
    );
    assert_eq!(calls.get(), 1);
    maestro_web::set_show_json_mode(true);
    let error = render_tool::<_, Value>(
        "raw-object",
        Some(ToolParams::typed(&object, format)),
        None,
        true,
    )
    .err()
    .unwrap();
    assert_eq!(error.to_string(), "formatter reached");
    assert_eq!(calls.get(), 1);
    maestro_web::set_show_json_mode(false);
}

#[test]
fn model_tool_call_uses_default_raw_object_route() {
    reset();
    for (index, arguments) in [json!({}), json!({"nested": [null, false, {"value": 7}]})]
        .into_iter()
        .enumerate()
    {
        let call: maestro_app::ToolCall = serde_json::from_value(json!({"type":"toolCall", "id":"model-call", "name":"model-route", "arguments": arguments})).unwrap();
        let address = std::ptr::from_ref(&call.arguments) as usize;
        let called = Rc::new(Cell::new(false));
        let observed = called.clone();
        let renderer: Rc<dyn ToolRenderer> = Rc::new(Renderer(Rc::new(
            move |params: Option<&maestro_app::JsonObject>,
                  _: Option<ToolResultMessageView<'_>>,
                  _| {
                assert_eq!(std::ptr::from_ref(params.unwrap()) as usize, address);
                assert_eq!(params.unwrap().is_empty(), index == 0);
                observed.set(true);
                Ok(content("model", true))
            },
        )));
        register_tool_renderer(&call.name, ToolRendererBinding::from_raw(renderer));
        assert!(
            render_tool::<_, Value>(
                &call.name,
                Some(ToolParams::object(
                    call.arguments.as_object().expect("recorded object root"),
                )),
                None,
                false
            )
            .unwrap()
            .is_custom
        );
        assert!(called.get());
        let typed: Rc<dyn ToolRenderer<Value>> = Rc::new(Renderer(Rc::new(
            |params: Option<&Value>, _: Option<ToolResultMessageView<'_>>, _| {
                assert!(params.unwrap().is_object());
                Ok(content("typed value", true))
            },
        )));
        register_tool_renderer(&call.name, ToolRendererBinding::from_typed(typed));
        assert!(
            render_tool::<_, Value>(
                &call.name,
                Some(ToolParams::object(
                    call.arguments.as_object().expect("recorded object root"),
                )),
                None,
                false
            )
            .unwrap()
            .is_custom
        );
    }
}

#[test]
fn native_operands_and_non_object_sdk_values_remain_available() {
    reset();
    let native = Rc::new(Cell::new(41));
    let expected = native.clone();
    register_tool_renderer(
        "native-object-control",
        ToolRendererBinding::from_raw(Rc::new(Renderer(Rc::new(
            move |params: Option<&Rc<Cell<i32>>>,
                  result: Option<ToolResultMessageView<'_, Rc<Cell<i32>>>>,
                  _| {
                assert!(Rc::ptr_eq(params.unwrap(), &expected));
                assert!(Rc::ptr_eq(result.unwrap().details().unwrap(), &expected));
                expected.set(expected.get() + 1);
                Ok(content("native", true))
            },
        )))),
    );
    let envelope = message(Some(native.clone()));
    assert!(
        render_tool(
            "native-object-control",
            Some(ToolParams::typed(&native, |_| panic!(
                "native callback formatted input"
            ))),
            Some(&envelope),
            false
        )
        .unwrap()
        .is_custom
    );
    assert_eq!(native.get(), 42);
    non_object_sdk_values();
}

/// The default listing preserves canonical numeric keys, number spelling and literal Unicode.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn default_object_input_keeps_canonical_json_spelling(import_module: &js_sys::Function) {
    use crate::contributions_support::{container, host, select, text, view};
    use maestro_web::dom::MountedContent;
    reset();
    let object: maestro_app::JsonObject = serde_json::from_str(
        r#"{"b":"雪 <literal>","10":10,"2":2,"zero":-0.0,"large":1e21,"small":1e-7}"#,
    )
    .unwrap();
    let recipe = render_tool::<_, Value>(
        "canonical-object",
        Some(ToolParams::object(&object)),
        None,
        false,
    )
    .unwrap();
    let view = view();
    let _elements = host(&view, import_module);
    let parent = container();
    let mut slot = MountedContent::default();
    slot.show(recipe.content.as_ref(), &parent, &view).unwrap();
    let block = select(slot.root().unwrap(), ".maestro-code");
    assert_eq!(
        parent.query_selector_all(".maestro-code").unwrap().length(),
        1
    );
    assert_eq!(block.get_attribute("language").unwrap(), "json");
    assert_eq!(
        text(&select(&block, "pre code")),
        "{\n  \"2\": 2,\n  \"10\": 10,\n  \"b\": \"雪 <literal>\",\n  \"zero\": 0,\n  \"large\": 1e+21,\n  \"small\": 1e-7\n}"
    );
    slot.clear();
    parent.remove();
}

/// Tool rows borrow every argument root and retain native result details.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn tool_rows_dispatch_borrowed_argument_root_values() {
    reset();
    let native = Rc::new(Cell::new(0));
    let envelope = message(Some(native.clone()));
    for arguments in [
        json!(null),
        json!(false),
        json!(0),
        json!(""),
        json!("text"),
        json!([]),
        json!([null, {"k": 1}]),
        json!({}),
        json!({"z": 1, "a": [false]}),
    ] {
        let call = Rc::new(model_call("root-dispatch", &arguments));
        for (pending, aborted, is_streaming) in [
            (false, false, false),
            (true, false, false),
            (false, false, true),
            (true, true, true),
        ] {
            let streaming = !aborted && (pending || is_streaming);
            let calls = Rc::new(Cell::new(0));
            let id = if aborted {
                "root-call"
            } else {
                "original call"
            };
            register_root_observer(&call, &calls, &native, streaming, id);
            let props = maestro_web::ToolMessage {
                tool_call: Some(call.clone()),
                tool: None,
                result: Some(Rc::new(envelope.clone())),
                pending,
                aborted,
                is_streaming,
            };
            assert!(draw_row(&props).is_empty(), "custom rows draw no listing");
            assert_eq!(calls.get(), 1, "{} {pending} {aborted}", call.arguments);
        }
    }
    assert_eq!(
        native.get(),
        9 * 3,
        "aborted rows replace the native envelope"
    );
}

/// Default tool rows retain root-specific input and output listings.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn tool_rows_keep_root_specific_default_input_sections() {
    reset();
    let output = maestro_app::ToolResultMessage::<Value> {
        content: vec![maestro_app::UserBlock::Text(maestro_app::TextContent::new(
            r#"{"ok":true}"#.into(),
        ))],
        ..message(None)
    };
    let empty = maestro_app::ToolResultMessage::<Value> {
        content: Vec::new(),
        is_error: false,
        ..message(None)
    };
    let ok = "{\n  \"ok\": true\n}";
    let cases: [(
        Value,
        Option<&maestro_app::ToolResultMessage>,
        bool,
        Vec<&str>,
    ); 17] = [
        (json!(null), None, false, vec![]),
        (json!(false), None, false, vec![]),
        (json!(0), None, false, vec![]),
        (json!(""), None, false, vec![]),
        (json!(true), None, false, vec!["true"]),
        (json!([]), None, true, vec!["[]"]),
        (json!([5]), None, false, vec!["[\n  5\n]"]),
        (json!([null]), None, false, vec!["[\n  null\n]"]),
        (
            json!(r#"{"b":1,"a":[2]}"#),
            None,
            false,
            vec!["{\n  \"b\": 1,\n  \"a\": [\n    2\n  ]\n}"],
        ),
        (json!(r#""quoted""#), None, false, vec!["\"quoted\""]),
        (json!("plain"), None, false, vec!["\"plain\""]),
        (json!("null"), None, true, vec![]),
        (json!({}), None, true, vec![]),
        (json!({}), None, false, vec!["{}"]),
        (json!(null), Some(&output), false, vec![ok]),
        (json!({}), Some(&output), true, vec!["{}", ok]),
        (json!({}), Some(&empty), false, vec!["{}"]),
    ];
    for custom in [false, true] {
        if custom {
            show_json_over_custom("default-sections");
        }
        for (arguments, result, is_streaming, expected) in &cases {
            let call = Rc::new(model_call("default-sections", arguments));
            let props = maestro_web::ToolMessage {
                tool_call: Some(call.clone()),
                tool: None,
                result: result.map(|value| Rc::new(value.clone())),
                pending: false,
                aborted: false,
                is_streaming: *is_streaming,
            };
            assert_eq!(draw_row(&props), *expected, "{arguments} {is_streaming}");
        }
    }
    maestro_web::set_show_json_mode(false);
}

/// Register a custom renderer, then enable JSON mode so the default sections still render.
#[cfg(target_arch = "wasm32")]
fn show_json_over_custom(name: &str) {
    register_tool_renderer(
        name,
        ToolRendererBinding::from_raw(Rc::new(Renderer(Rc::new(
            |_: Option<&Value>, _: Option<ToolResultMessageView<'_>>, _| {
                Ok(content("custom", true))
            },
        )))),
    );
    maestro_web::set_show_json_mode(true);
}

/// A retained model call holding the supplied argument value.
#[cfg(target_arch = "wasm32")]
fn model_call(name: &str, arguments: &Value) -> maestro_app::ToolCall {
    serde_json::from_value(
        json!({"type": "toolCall", "id": "root-call", "name": name, "arguments": arguments}),
    )
    .unwrap()
}

/// Register the call's operand route and check each callback borrows the original value.
#[cfg(target_arch = "wasm32")]
fn register_root_observer(
    call: &maestro_app::ToolCall,
    calls: &Rc<Cell<usize>>,
    native: &Rc<Cell<i32>>,
    streaming: bool,
    id: &'static str,
) {
    let address = match call.arguments.as_object() {
        Some(members) => std::ptr::from_ref(members) as usize,
        None => std::ptr::from_ref(&call.arguments) as usize,
    };
    let expected = call.arguments.clone();
    let (count, native) = (calls.clone(), native.clone());
    let check = move |address_seen: usize,
                      seen: Value,
                      result: Option<ToolResultMessageView<'_, Rc<Cell<i32>>>>,
                      flag: bool| {
        assert_eq!(address_seen, address);
        assert_eq!(seen, expected);
        assert_eq!(flag, streaming);
        let result = result.unwrap();
        assert_eq!(result.tool_call_id(), id);
        if let Some(details) = result.details() {
            assert!(Rc::ptr_eq(details, &native));
            native.set(native.get() + 1);
        }
        count.set(count.get() + 1);
        Ok(content("root", true))
    };
    let binding = if call.arguments.is_object() {
        ToolRendererBinding::from_raw(Rc::new(Renderer(Rc::new(
            move |params: Option<&Map<String, Value>>, result, flag| {
                let params = params.unwrap();
                check(
                    std::ptr::from_ref(params) as usize,
                    Value::Object(params.clone()),
                    result,
                    flag,
                )
            },
        ))))
    } else {
        ToolRendererBinding::from_raw(Rc::new(Renderer(Rc::new(
            move |params: Option<&Value>, result, flag| {
                let params = params.unwrap();
                check(
                    std::ptr::from_ref(params) as usize,
                    params.clone(),
                    result,
                    flag,
                )
            },
        ))))
    };
    register_tool_renderer(&call.name, binding);
}

/// Mount one tool row and read its JSON listings in order.
#[cfg(target_arch = "wasm32")]
fn draw_row<D: 'static>(props: &maestro_web::ToolMessage<D>) -> Vec<String> {
    use crate::contributions_support::{body, text, view};
    use maestro_web::dom::DomContent as _;
    use wasm_bindgen::JsCast as _;
    let view = view();
    let root = props.mount(&view).unwrap();
    body().append_child(&root.node).unwrap();
    let element = root.node.unchecked_ref::<web_sys::Element>();
    let blocks = element
        .query_selector_all(".maestro-code[language='json'] pre code")
        .unwrap();
    let listings = (0..blocks.length())
        .map(|index| text(&blocks.item(index).unwrap()))
        .collect();
    element.remove();
    listings
}

/// Register the typed callback with the original envelope addresses.
fn register_typed_object(calls: &Rc<Cell<usize>>, result: &maestro_app::ToolResultMessage) {
    let count = calls.clone();
    let blocks = result.content.as_ptr() as usize;
    let id = result.tool_call_id.as_ptr() as usize;
    register_tool_renderer(
        "object-admission",
        ToolRendererBinding::from_typed(Rc::new(Renderer(Rc::new(
            move |params: Option<&Arguments>,
                  result: Option<ToolResultMessageView<'_, Arguments>>,
                  streaming| {
                assert_eq!(params.unwrap().message, "producer value");
                let result = result.unwrap();
                assert_eq!(result.role(), "toolResult");
                assert_eq!(result.tool_call_id(), "original call");
                assert_eq!(result.tool_name(), "original tool");
                assert_eq!(result.timestamp().to_bits(), 17.25_f64.to_bits());
                assert!(result.is_error() && streaming);
                assert_eq!(result.content().as_ptr() as usize, blocks);
                assert_eq!(result.tool_call_id().as_ptr() as usize, id);
                if let Some(details) = result.details() {
                    assert_eq!(details.message, "detail");
                }
                count.set(count.get() + 1);
                Ok(content("typed object", true))
            },
        )))),
    );
}

/// Exercise the explicit JSON-value registration after native admission.
fn non_object_sdk_values() {
    let calls = Rc::new(Cell::new(0));
    for value in [
        None,
        Some(Value::Bool(false)),
        Some(json!(0)),
        Some(json!("")),
        Some(Value::Null),
        Some(json!({})),
        Some(json!([1, "sdk"])),
    ] {
        let expected = value.clone();
        let observed = calls.clone();
        register_tool_renderer(
            "sdk-value",
            ToolRendererBinding::from_raw(Rc::new(Renderer(Rc::new(
                move |params: Option<&Value>, _: Option<ToolResultMessageView<'_>>, _| {
                    assert_eq!(params, expected.as_ref());
                    observed.set(observed.get() + 1);
                    Ok(content("SDK value", true))
                },
            )))),
        );
        let before = calls.get();
        assert!(
            render_tool::<_, Value>(
                "sdk-value",
                value.as_ref().map(ToolParams::json),
                None,
                false
            )
            .unwrap()
            .is_custom
        );
        assert_eq!(calls.get(), before + 1);
    }
}

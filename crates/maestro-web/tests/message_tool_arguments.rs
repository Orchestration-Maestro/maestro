//! Model object admission through the public browser tool dispatch.
#![cfg(test)]
use crate::support::tool_output::TextOnlyOutput as _;
use crate::{brand, effects_support, recording_highlighter, support, tool_renderer_support};

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
                Some(ToolParams::object(&call.arguments)),
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
                Some(ToolParams::object(&call.arguments)),
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

#[test]
fn default_object_input_keeps_canonical_json_spelling() {
    use maestro_web::browser::attachment_content::AttachmentHost;
    use maestro_web::tools::types::{ToolRenderState, ToolRenderView};
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
    let highlighter = recording_highlighter::Highlighter::default();
    let host: Rc<dyn AttachmentHost> = Rc::new(effects_support::Host::default());
    let i18n = maestro_web::I18n::new(Rc::new(support::Host::default())).unwrap();
    let pack = maestro_theme::load_brand_pack("brand.json", &brand::Pack).unwrap();
    let presentation = pack.presentation(maestro_theme::BrandMode::Dark).unwrap();
    let context = egui::Context::default();
    let mut row = ToolRenderState::default();
    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(600., 600.),
            )),
            ..Default::default()
        },
        |ui| {
            recipe
                .render(
                    ui,
                    ToolRenderView::new(&host, &i18n, &presentation, &highlighter, None),
                    &mut row,
                )
                .assert_no_overlays()
                .unwrap();
        },
    );
    output.textures_delta.clear();
    assert_eq!(&*highlighter.requests.borrow(), &[("{\n  \"2\": 2,\n  \"10\": 10,\n  \"b\": \"雪 <literal>\",\n  \"zero\": 0,\n  \"large\": 1e+21,\n  \"small\": 1e-7\n}".into(), "json".into())]);
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

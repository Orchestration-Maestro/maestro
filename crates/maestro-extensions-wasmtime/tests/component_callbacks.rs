//! Retained component views keep their operands, live state and callback scope.
#![cfg(test)]
use maestro_extensions::{ComponentHandle, EventError, MessageRenderOptions, MessageRendererCall};
use serde_json::{Value, json};
use std::rc::Rc;

use super::callback_ordering::{Instance, Pair, local};

/// Calls a retained renderer with a complete message built from `message`.
fn render(
    call: &MessageRendererCall<Value, maestro_extensions::Local>,
    message: &Value,
    expanded: bool,
) -> Result<Option<ComponentHandle>, EventError> {
    call.call(
        Rc::new(serde_json::from_value(message.clone()).unwrap()),
        MessageRenderOptions { expanded },
        maestro_theme::LiveTheme::default(),
    )
}

/// Looks up `custom_type`'s renderer in `instance` through the runner.
fn renderer(
    instance: &Instance,
    custom_type: &str,
) -> MessageRendererCall<Value, maestro_extensions::Local> {
    instance
        .runner
        .get_message_renderer_call::<Value>(custom_type)
        .unwrap()
}

/// A complete message of `custom_type`.
fn message(custom_type: &str) -> Value {
    json!({"role":"custom", "customType":custom_type, "content":"", "display":true, "timestamp":0})
}

#[test]
fn registered_component_renderer_keeps_message_options_and_theme() {
    local(async {
        let pair = Pair::new().await;
        let runner = &pair.a.runner;
        for missing in ["Echo-view", "echo-view ", "echo", ""] {
            assert!(
                runner.get_message_renderer_call::<Value>(missing).is_none(),
                "{missing:?}"
            );
        }
        let call = renderer(&pair.a, "echo-view");
        let blocks = json!([
            {"type":"text", "text":"first \u{feff}"},
            {"type":"image", "data":"AAAA", "mimeType":"image/png"},
            {"type":"text", "text":""},
        ]);
        let messages = [
            json!({"role":"custom", "customType":"component", "content":" literal \u{85} text ", "display":true, "timestamp":12.5}),
            json!({"role":"custom", "customType":"none", "content":blocks, "display":false, "details":null, "timestamp":-0.0}),
            json!({"role":"custom", "customType":"error", "content":"", "display":true, "details":false, "timestamp":0}),
            json!({"role":"custom", "customType":"component", "content":"", "display":true, "details":"", "timestamp":1}),
            json!({"role":"custom", "customType":"component", "content":"", "display":true, "details":[0, -1.5], "timestamp":1}),
            json!({"role":"custom", "customType":"component", "content":"", "display":true, "details":{"z":null, "a":{}}, "timestamp":1}),
        ];
        for (index, supplied) in messages.iter().enumerate() {
            let expanded = index % 2 == 1;
            let outcome = render(&call, supplied, expanded);
            match supplied["customType"].as_str().unwrap() {
                "none" => assert!(outcome.unwrap().is_none()),
                "error" => assert_eq!(outcome.err().unwrap().to_string(), "renderer refused"),
                _ => assert!(outcome.unwrap().is_some()),
            }
            let forwarded = pair.payloads.borrow_mut().pop().unwrap();
            let parsed: maestro_session::messages::CustomMessage<Value> =
                serde_json::from_value(supplied.clone()).unwrap();
            let expected = serde_json::to_value(&parsed).unwrap();
            assert_eq!(
                forwarded["message"].to_string(),
                expected.to_string(),
                "{index}"
            );
            assert_eq!(forwarded["expanded"], expanded, "{index}");
        }
        assert!(pair.payloads.borrow().is_empty());
        render(&call, &messages[1], false).unwrap();
        let forwarded = pair.payloads.borrow_mut().pop().unwrap();
        assert!(
            forwarded["message"]["timestamp"]
                .as_f64()
                .unwrap()
                .is_sign_negative()
        );
        assert!(forwarded["message"].get("details").is_some());
        render(&call, &messages[0], false).unwrap();
        let forwarded = pair.payloads.borrow_mut().pop().unwrap();
        assert!(forwarded["message"].get("details").is_none());
    });
}

#[test]
fn component_methods_share_retained_state_and_callback_scope() {
    local(async {
        let pair = Pair::new().await;
        let view = render(&renderer(&pair.a, "echo-view"), &message("component"), true)
            .unwrap()
            .unwrap();
        let lines = |width: usize, inputs: &str, invalidated: u32| {
            vec![
                format!("width:{width}"),
                String::new(),
                "dup".into(),
                "dup".into(),
                "\u{1b}[1m\u{96ea}\u{1b}[22m".into(),
                format!("inputs:{inputs}"),
                format!("invalidated:{invalidated}"),
            ]
        };
        for width in [0, 1, 2, 47] {
            assert_eq!(view.try_render(width).unwrap(), lines(width, "", 0));
        }
        let input = view.input_handler().unwrap();
        input.handle_input("\u{1b}[A");
        input.handle_input(" literal \u{96ea} ");
        view.invalidate();
        assert_eq!(
            view.try_render(3).unwrap(),
            lines(3, "\u{1b}[A| literal \u{96ea} ", 1)
        );
        each_method_returns_before_delivery_ends(&pair).await;
    });
}

/// Each method's same-instance listener runs inside it; the other instance's after it returns.
async fn each_method_returns_before_delivery_ends(pair: &Pair) {
    pair.a
        .program(r#"[{"op":"on","channel":"component-foreign","id":"same","name":"same"}]"#)
        .await;
    pair.b
        .program(r#"[{"op":"on","channel":"component-foreign","id":"foreign","name":"foreign"}]"#)
        .await;
    pair.take();
    let scoped = returned(pair, || {
        render(&renderer(&pair.a, "scoped-view"), &message("scoped"), false)
            .unwrap()
            .unwrap()
    });
    assert_eq!(
        returned(pair, || scoped.try_render(9).unwrap()),
        ["scoped:9"]
    );
    returned(pair, || scoped.input_handler().unwrap().handle_input("x"));
    returned(pair, || scoped.invalidate());
    let check = |method: &str| {
        [
            format!("a:{method}:start"),
            "same:prefix".into(),
            format!("a:{method}:end"),
            "foreign:prefix".into(),
            "caller:return".into(),
        ]
    };
    let trace = pair.take();
    let expected: Vec<String> = ["create", "render", "input", "invalidate"]
        .into_iter()
        .flat_map(check)
        .collect();
    assert_eq!(trace, expected);
}

/// Runs one component call and marks its return in the trace.
fn returned<T>(pair: &Pair, call: impl FnOnce() -> T) -> T {
    let result = call();
    pair.trace.borrow_mut().push("caller:return".into());
    result
}

#[test]
fn component_defaults_and_release_preference_remain_live() {
    local(async {
        let pair = Pair::new().await;
        pair.b
            .program(
                r#"[{"op":"on","channel":"component-foreign","id":"foreign","name":"foreign"}]"#,
            )
            .await;
        let call = renderer(&pair.a, "echo-view");
        let plain = render(&call, &message("plain"), false).unwrap().unwrap();
        plain.input_handler().unwrap().handle_input("ignored");
        assert!(!plain.wants_key_release());
        assert_eq!(plain.try_render(4).unwrap(), ["renders:1"]);
        let preference = render(&call, &message("preference"), false)
            .unwrap()
            .unwrap();
        pair.take();
        let mut observed = Vec::new();
        for input in [None, Some("on"), Some("on"), Some("off")] {
            if let Some(input) = input {
                preference.input_handler().unwrap().handle_input(input);
            }
            observed.push(preference.wants_key_release());
            pair.trace.borrow_mut().push("caller:return".into());
        }
        assert_eq!(observed, [false, true, true, false]);
        let frame = ["a:preference", "foreign:prefix", "caller:return"];
        assert_eq!(pair.take(), frame.repeat(4));
        assert_eq!(preference.try_render(1).unwrap(), ["release:false"]);
    });
}

#[test]
fn returned_components_outlive_renderer_registration() {
    local(async {
        let pair = Pair::new().await;
        let original = renderer(&pair.a, "swap-view");
        let first = render(&original, &message("keep"), false).unwrap().unwrap();
        let alias = first.clone();
        pair.a.program(r#"[{"op":"swap","version":"v2"}]"#).await;
        let replacement = render(&renderer(&pair.a, "swap-view"), &message("keep"), false)
            .unwrap()
            .unwrap();
        let retained = render(&original, &message("old"), false).unwrap().unwrap();
        assert_eq!(first.try_render(3).unwrap(), ["keep:v1:3"]);
        assert_eq!(alias.try_render(4).unwrap(), ["keep:v1:4"]);
        assert_eq!(replacement.try_render(5).unwrap(), ["keep:v2:5"]);
        assert_eq!(retained.try_render(6).unwrap(), ["old:v1:6"]);
        drop(first);
        pair.a.barrier().await;
        assert!(!pair.a.released("keep:v1"));
        assert_eq!(alias.try_render(7).unwrap(), ["keep:v1:7"]);
        drop(alias);
        pair.a.barrier().await;
        assert!(pair.a.released("keep:v1"));
        assert!(!pair.a.released("keep:v2") && !pair.a.released("old:v1"));
        drop((replacement, retained));
        pair.a.barrier().await;
        assert!(pair.a.released("keep:v2") && pair.a.released("old:v1"));
    });
}

#[test]
fn component_callbacks_drain_before_release() {
    local(async {
        let pair = Pair::new().await;
        pair.b
            .program(
                r#"[{"op":"on","channel":"component-foreign","id":"foreign","name":"foreign"}]"#,
            )
            .await;
        pair.take();
        let view = returned(&pair, || {
            render(&renderer(&pair.a, "scoped-view"), &message("scoped"), false)
                .unwrap()
                .unwrap()
        });
        returned(&pair, || view.try_render(2).unwrap());
        returned(&pair, || view.input_handler().unwrap().handle_input("x"));
        assert!(returned(&pair, || view.wants_key_release()));
        returned(&pair, || view.invalidate());
        let expected: Vec<String> = ["create", "render", "input", "preference", "invalidate"]
            .into_iter()
            .flat_map(|method| {
                [
                    format!("a:{method}:start"),
                    format!("a:{method}:end"),
                    "foreign:prefix".into(),
                    "caller:return".into(),
                ]
            })
            .collect();
        assert_eq!(pair.take(), expected);
        assert!(!pair.a.released("scoped"));
        drop(view);
        pair.a.barrier().await;
        assert!(pair.a.released("scoped"));
        assert!(pair.take().is_empty());
    });
}

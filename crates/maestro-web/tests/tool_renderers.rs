//! Browser tool dispatch through retained typed registrations.
#![cfg(test)]
use maestro_web::tools::types::ToolUiError;
use maestro_web::tools::{
    renderer_registry::{TOOL_RENDERERS, get_tool_renderer, register_tool_renderer},
    types::{ToolRenderResult, ToolRenderer, ToolRendererBinding, ToolResultMessageView},
};
use serde_json::Value;
use std::rc::Rc;

struct Renderer(&'static str);
impl ToolRenderer<Value> for Renderer {
    fn render(
        &self,
        _: Option<&Value>,
        _: Option<ToolResultMessageView<'_>>,
        _: bool,
    ) -> Result<ToolRenderResult, ToolUiError> {
        Err(ToolUiError::Host(
            maestro_web::browser::attachment_content::AttachmentHostError::Host(self.0.into()),
        ))
    }
}

#[test]
fn registry_preserves_replacement_and_retained_handles() {
    TOOL_RENDERERS.with(|all| all.borrow_mut().clear());
    assert!(get_tool_renderer::<Value, Value>("missing").is_none());
    let renderer: Rc<dyn ToolRenderer<Value>> = Rc::new(Renderer("original"));
    for name in ["", "雪", "__proto__"] {
        register_tool_renderer(name, ToolRendererBinding::from_raw(renderer.clone()));
    }
    let retained = get_tool_renderer::<Value, Value>("雪").unwrap();
    let replacement: Rc<dyn ToolRenderer<Value>> = Rc::new(Renderer("replacement"));
    register_tool_renderer("雪", ToolRendererBinding::from_raw(replacement.clone()));
    let current = get_tool_renderer::<Value, Value>("雪").unwrap();
    assert!(Rc::ptr_eq(&current, &replacement));
    TOOL_RENDERERS.with(|all| {
        assert_eq!(
            all.borrow().keys().map(String::as_str).collect::<Vec<_>>(),
            ["", "雪", "__proto__"]
        );
        all.borrow_mut().shift_remove("雪");
    });
    assert_eq!(
        retained
            .render(None, None, false)
            .err()
            .unwrap()
            .to_string(),
        "original"
    );
    assert_eq!(
        current.render(None, None, false).err().unwrap().to_string(),
        "replacement"
    );
}

use crate::tool_renderer_support;
use maestro_web::{render_tool, set_show_json_mode, tools::types::ToolParams};
use std::cell::{Cell, RefCell};
use tool_renderer_support::{Renderer as CallbackRenderer, content, message, reset};

#[test]
fn tool_dispatch_forwards_complete_arguments_and_custom_wrapper_choice() {
    reset();
    tool_renderer_support::forwarding::run();
    register_tool_renderer(
        "failure",
        ToolRendererBinding::from_raw::<Value, Value>(Rc::new(Renderer("controlled"))),
    );
    assert!(
        matches!(render_tool::<Value,Value>("failure",None,None,false),Err(ToolUiError::Host(maestro_web::browser::attachment_content::AttachmentHostError::Host(message))) if message == "controlled")
    );
}

struct Native {
    value: Rc<Cell<u32>>,
    formatted: Rc<Cell<usize>>,
}
fn format_native(input: &Native) -> Result<Option<String>, ToolUiError> {
    input.formatted.set(input.formatted.get() + 1);
    maestro_app::pretty_json(&serde_json::json!(input.value.get()))
        .map(Some)
        .map_err(|error| ToolUiError::Presentation(error.to_string()))
}
struct Details(Rc<Cell<u32>>);
#[test]
fn native_tool_renderers_keep_non_json_handles_and_lazy_default_formatting() {
    reset();
    let handle = Rc::new(Cell::new(42));
    let formatted = Rc::new(Cell::new(0));
    let input = Native {
        value: handle.clone(),
        formatted: formatted.clone(),
    };
    let result = message(Some(Details(handle.clone())));
    let seen = Rc::new(RefCell::new(Vec::new()));
    let output = seen.clone();
    let expected = handle.clone();
    let renderer: Rc<dyn ToolRenderer<Native, Details>> = Rc::new(CallbackRenderer(Rc::new(
        move |params: Option<&Native>,
              result: Option<ToolResultMessageView<'_, Details>>,
              stream| {
            assert!(stream);
            assert!(Rc::ptr_eq(&params.unwrap().value, &expected));
            let result = result.unwrap();
            assert!(Rc::ptr_eq(&result.details().unwrap().0, &expected));
            output.borrow_mut().push(expected.get());
            Ok(content("retained", true))
        },
    )));
    register_tool_renderer("native", ToolRendererBinding::from_raw(renderer.clone()));
    assert!(Rc::ptr_eq(
        &get_tool_renderer::<Native, Details>("native").unwrap(),
        &renderer
    ));
    for value in [42, 43, 44] {
        handle.set(value);
        assert!(
            render_tool(
                "native",
                Some(ToolParams::typed(&input, format_native)),
                Some(&result),
                true
            )
            .unwrap()
            .is_custom
        );
    }
    assert_eq!(*seen.borrow(), [42, 43, 44]);
    assert_eq!(formatted.get(), 0);
    set_show_json_mode(true);
    assert!(
        !render_tool(
            "native",
            Some(ToolParams::typed(&input, format_native)),
            Some(&result),
            true
        )
        .unwrap()
        .is_custom
    );
    assert_eq!(formatted.get(), 1);
    assert_eq!(*seen.borrow(), [42, 43, 44]);
    set_show_json_mode(false);
    assert!(render_tool::<Value, Value>("native", None, None, false).is_err());
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Decoded {
    value: u32,
}
#[test]
fn typed_tool_renderer_json_admission_preserves_borrowed_message_fields() {
    reset();
    let message = message(Some(serde_json::json!({"value":43})));
    let id = message.tool_call_id.as_ptr() as usize;
    let blocks = message.content.as_ptr() as usize;
    let seen = Rc::new(Cell::new(0));
    let seen_callback = seen.clone();
    let renderer: Rc<dyn ToolRenderer<Decoded, Decoded>> = Rc::new(CallbackRenderer(Rc::new(
        move |p: Option<&Decoded>,
              result: Option<ToolResultMessageView<'_, Decoded>>,
              streaming| {
            let result = result.unwrap();
            assert_eq!(p.unwrap().value, 42);
            assert_eq!(result.details().unwrap().value, 43);
            assert_eq!(result.role(), "toolResult");
            assert_eq!(result.tool_call_id(), "original call");
            assert_eq!(result.tool_name(), "original tool");
            assert_eq!(result.timestamp().to_bits(), 17.25_f64.to_bits());
            assert!(result.is_error() && streaming);
            assert_eq!(result.tool_call_id().as_ptr() as usize, id);
            assert_eq!(result.content().as_ptr() as usize, blocks);
            seen_callback.set(seen_callback.get() + 1);
            Ok(content("decoded", true))
        },
    )));
    register_tool_renderer("typed", ToolRendererBinding::from_typed(renderer.clone()));
    assert!(Rc::ptr_eq(
        &get_tool_renderer::<Decoded, Decoded>("typed").unwrap(),
        &renderer
    ));
    assert!(
        render_tool(
            "typed",
            Some(ToolParams::json(&serde_json::json!({"value":42}))),
            Some(&message),
            true
        )
        .unwrap()
        .is_custom
    );
    assert_eq!(seen.get(), 1);
    let raw: Rc<dyn ToolRenderer<Value>> = Rc::new(Renderer("original"));
    let binding = ToolRendererBinding::from_typed(raw.clone());
    assert!(Rc::ptr_eq(
        &binding.renderer::<Value, Value>().unwrap(),
        &raw
    ));
}

#[test]
fn tool_renderer_decoder_failures_do_not_narrow_raw_registration() {
    reset();
    let calls = Rc::new(Cell::new(0));
    let count = calls.clone();
    register_tool_renderer(
        "decode",
        ToolRendererBinding::from_typed(Rc::new(CallbackRenderer(Rc::new(
            move |p: Option<&Decoded>, r: Option<ToolResultMessageView<'_, Decoded>>, _| {
                if let Some(p) = p {
                    assert_eq!(p.value, 42);
                }
                if let Some(d) = r.as_ref().and_then(ToolResultMessageView::details) {
                    assert_eq!(d.value, 42);
                }
                count.set(count.get() + 1);
                Ok(content("decoded", false))
            },
        )))),
    );
    for params in [
        None,
        Some(Value::Null),
        Some(serde_json::json!({"value":42})),
        Some(serde_json::json!({"value":"bad"})),
    ] {
        for details in [
            None,
            Some(Value::Null),
            Some(serde_json::json!({"value":42})),
            Some(serde_json::json!({"value":"bad"})),
        ] {
            let valid = params
                .as_ref()
                .is_none_or(|p| p.get("value") == Some(&serde_json::json!(42)))
                && details
                    .as_ref()
                    .is_none_or(|p| p.get("value") == Some(&serde_json::json!(42)));
            let previous = calls.get();
            let message = message(details);
            let result = render_tool(
                "decode",
                params.as_ref().map(ToolParams::json),
                Some(&message),
                false,
            );
            assert_eq!(result.is_ok(), valid);
            assert_eq!(calls.get() - previous, usize::from(valid));
        }
    }
    raw_registration_after_decoder_failures();
}

#[test]
fn json_mode_bypasses_custom_renderers_without_unregistering() {
    reset();
    let calls = Rc::new(Cell::new(0));
    let count = calls.clone();
    let renderer: Rc<dyn ToolRenderer<Value>> = Rc::new(CallbackRenderer(Rc::new(
        move |_: Option<&Value>, _: Option<ToolResultMessageView<'_>>, _| {
            count.set(count.get() + 1);
            Ok(content("custom", true))
        },
    )));
    register_tool_renderer("custom", ToolRendererBinding::from_raw(renderer.clone()));
    assert!(
        render_tool::<Value, Value>("custom", None, None, false)
            .unwrap()
            .is_custom
    );
    set_show_json_mode(true);
    assert!(
        !render_tool::<Value, Value>("custom", None, None, false)
            .unwrap()
            .is_custom
    );
    assert!(
        !render_tool::<Value, Value>("unknown", None, None, false)
            .unwrap()
            .is_custom
    );
    assert!(Rc::ptr_eq(
        &get_tool_renderer::<Value, Value>("custom").unwrap(),
        &renderer
    ));
    assert_eq!(calls.get(), 1);
    set_show_json_mode(false);
    assert!(
        render_tool::<Value, Value>("custom", None, None, false)
            .unwrap()
            .is_custom
    );
    assert_eq!(calls.get(), 2);
}

#[test]
fn renderer_callback_can_replace_registry_entries() {
    reset();
    register_tool_renderer(
        "reentrant",
        ToolRendererBinding::from_raw(Rc::new(CallbackRenderer(Rc::new(
            |_: Option<&Value>, _: Option<ToolResultMessageView<'_>>, _| {
                let old = TOOL_RENDERERS.with(|all| all.borrow_mut().shift_remove("reentrant"));
                drop(old);
                register_tool_renderer(
                    "reentrant",
                    ToolRendererBinding::from_raw(Rc::new(Renderer("replacement"))),
                );
                register_tool_renderer(
                    "other",
                    ToolRendererBinding::from_raw(Rc::new(Renderer("other"))),
                );
                set_show_json_mode(true);
                Ok(content("current callback", true))
            },
        )))),
    );
    assert!(
        render_tool::<Value, Value>("reentrant", None, None, false)
            .unwrap()
            .is_custom
    );
    assert!(
        !render_tool::<Value, Value>("reentrant", None, None, false)
            .unwrap()
            .is_custom
    );
    set_show_json_mode(false);
    assert_eq!(
        render_tool::<Value, Value>("reentrant", None, None, false)
            .err()
            .unwrap()
            .to_string(),
        "replacement"
    );
    assert!(get_tool_renderer::<Value, Value>("other").is_some());
}

/// Raw admission remains available after independently selected decoder failures.
fn raw_registration_after_decoder_failures() {
    let input = Native {
        value: Rc::new(Cell::new(42)),
        formatted: Rc::new(Cell::new(0)),
    };
    register_tool_renderer(
        "raw",
        ToolRendererBinding::from_raw(Rc::new(CallbackRenderer(Rc::new(
            |p: Option<&Native>, _: Option<ToolResultMessageView<'_, Details>>, _| {
                assert_eq!(p.unwrap().value.get(), 42);
                Ok(content("raw", true))
            },
        )))),
    );
    assert!(
        render_tool::<Native, Details>(
            "raw",
            Some(ToolParams::typed(&input, format_native)),
            None,
            false
        )
        .unwrap()
        .is_custom
    );
}

/// A renderer that names the prepared-content parameter explicitly.
struct ExplicitOutput(Rc<Cell<u32>>);
impl ToolRenderer<Value, Value, tool_renderer_support::PreparedContent> for ExplicitOutput {
    fn render(
        &self,
        _: Option<&Value>,
        _: Option<ToolResultMessageView<'_>>,
        _: bool,
    ) -> Result<ToolRenderResult<tool_renderer_support::PreparedContent>, ToolUiError> {
        self.0.set(self.0.get() + 1);
        Ok(content("explicit", true))
    }
}

/// Defaulted and explicit prepared-content types use the same dispatcher.
#[test]
fn content_parameter_keeps_explicit_renderers_working() {
    reset();
    let calls = Rc::new(Cell::new(0));
    register_tool_renderer(
        "explicit",
        ToolRendererBinding::from_raw::<Value, Value>(Rc::new(ExplicitOutput(calls.clone()))),
    );
    let count = calls.clone();
    let defaulted: Rc<dyn ToolRenderer<Value>> = Rc::new(CallbackRenderer(Rc::new(
        move |_: Option<&Value>, _: Option<ToolResultMessageView<'_>>, _| {
            count.set(count.get() + 10);
            Ok(content("defaulted", false))
        },
    )));
    register_tool_renderer("defaulted", ToolRendererBinding::from_raw(defaulted));
    let explicit: ToolRenderResult =
        render_tool::<Value, Value>("explicit", None, None, false).unwrap();
    let defaulted = render_tool::<Value, Value>("defaulted", None, None, false).unwrap();
    assert!(explicit.is_custom);
    assert!(!defaulted.is_custom);
    assert_eq!(calls.get(), 11);
    assert!(
        !render_tool::<Value, Value>("unknown", None, None, false)
            .unwrap()
            .is_custom
    );
}

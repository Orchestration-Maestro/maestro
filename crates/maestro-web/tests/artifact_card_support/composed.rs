//! Registered artifact recipes in retained tool and assistant message rows.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::{
    markdown_support::{Surface, check, js},
    support::{reading_view::view, tool_output::TextOnlyOutput as _},
    tool_renderer_browser as drawing,
};
use maestro_app::{
    JsonObject, ToolCall, ToolResultMessage, browser::tools::documents::ArtifactsParams,
};
use maestro_web::{
    ArtifactsToolRenderer, AssistantMessage, AssistantMessageProps, ToolMessage, ToolMessageProps,
    ToolRenderer, register_tool_renderer, render_tool, set_show_json_mode,
    tools::{
        renderer_registry::TOOL_RENDERERS,
        types::{ToolParams, ToolRenderState, ToolRendererBinding},
    },
};
use serde_json::{Value, json};
use std::{cell::Cell, collections::HashMap, rc::Rc};

/// Nonserializable details whose callback must never run.
type Details = Rc<dyn Fn()>;

/// Observe every raw adapter and the final message containing release path.
/// # Panics
/// Panics if dispatch, retained drawing, or ignored details differ.
pub fn run() {
    let renderer = Rc::new(ArtifactsToolRenderer::new(None));
    let binding = ToolRendererBinding::from_raw::<JsonObject, Details>(renderer.clone())
        .with_raw::<Value, Details>(renderer.clone())
        .with_raw::<ArtifactsParams, Details>(renderer);
    let names = TOOL_RENDERERS.with(|all| all.borrow().keys().cloned().collect::<Vec<_>>());
    assert!(!names.iter().any(|name| name == "artifacts"));
    register_tool_renderer("artifacts", binding.clone());
    let mut surface = Surface::new();
    surface.time = Some(0.);
    let reads = Rc::new(Cell::new(0));
    let observed = reads.clone();
    let details: Details = Rc::new(move || observed.set(observed.get() + 1));
    let weak = Rc::downgrade(&details);
    let mut result = crate::recipes::message(Some(details));
    result.is_error = false;
    result.content = vec![maestro_app::UserBlock::Text(maestro_app::TextContent::new(
        "literal result".into(),
    ))];
    adapters(&mut surface, &result);
    let call = call();
    tool(&mut surface, &call, &mut result);
    assistant(&mut surface, &call, &mut result);
    registry(&mut surface, &call, &result, binding);
    assert_eq!(reads.get(), 0);
    drop(result);
    assert!(
        weak.upgrade().is_none(),
        "rows must not retain native result details"
    );
    drop(TOOL_RENDERERS.with(|all| all.borrow_mut().shift_remove("artifacts")));
    assert_eq!(
        TOOL_RENDERERS.with(|all| all.borrow().keys().cloned().collect::<Vec<_>>()),
        names
    );
}
fn call() -> ToolCall {
    serde_json::from_value(json!({"type":"toolCall","id":"artifact-call","name":"artifacts","arguments":{"command":"create","filename":"literal.rs","content":"literal code"}})).unwrap()
}
fn adapters(surface: &mut Surface, result: &ToolResultMessage<Details>) {
    let params = json!({"command":"create","filename":"literal.rs","content":"literal code"});
    let object: JsonObject = serde_json::from_value(params.clone()).unwrap();
    let native: ArtifactsParams = serde_json::from_value(params.clone()).unwrap();
    let recipes = [
        render_tool(
            "artifacts",
            Some(ToolParams::object(&object)),
            Some(result),
            false,
        )
        .unwrap(),
        render_tool(
            "artifacts",
            Some(ToolParams::json(&params)),
            Some(result),
            false,
        )
        .unwrap(),
        render_tool(
            "artifacts",
            Some(ToolParams::typed(&native, |_| {
                panic!("registered native operands must not be formatted")
            })),
            Some(result),
            false,
        )
        .unwrap(),
    ];
    for recipe in recipes {
        assert!(!recipe.is_custom);
        let mut state = ToolRenderState::default();
        drawing::draw(surface, &recipe, &mut state);
        drawing::draw(surface, &recipe, &mut state);
        check("document.querySelector('.maestro-tool-caption').textContent==='Created artifact'");
        crate::document_card_support::browser::open(surface, &recipe, &mut state);
        check("document.querySelector('.maestro-code code').textContent==='literal code'");
    }
}
fn draw_tool(
    surface: &mut Surface,
    row: &mut ToolMessage,
    call: &ToolCall,
    result: Option<&ToolResultMessage<Details>>,
) {
    let host = surface.host();
    for _ in 0..2 {
        surface.draw(|ui, frame| {
            row.render(
                ui,
                ToolMessageProps {
                    tool_call: call,
                    tool: None,
                    result,
                    pending: result.is_none(),
                    aborted: false,
                    is_streaming: false,
                },
                view(&host, frame),
            )
            .assert_no_overlays()
            .unwrap()
        });
    }
}
fn caption(expected: &str) {
    assert_eq!(
        js("document.querySelector('.maestro-tool-caption').textContent")
            .as_string()
            .as_deref(),
        Some(expected)
    );
}
fn tool(surface: &mut Surface, call: &ToolCall, result: &mut ToolResultMessage<Details>) {
    let mut row = ToolMessage::new();
    for (phase, expected) in [
        (0, "Creating artifact"),
        (1, "Created artifact"),
        (2, "Creating artifact"),
    ] {
        result.is_error = phase == 2;
        draw_tool(surface, &mut row, call, (phase != 0).then_some(result));
        caption(expected);
        let _ = js("document.querySelector('.maestro-tool-caption').click()");
        surface.time = Some(surface.time.unwrap() + 0.3);
        draw_tool(surface, &mut row, call, (phase != 0).then_some(result));
        check("document.querySelector('.maestro-artifact-pill').textContent==='literal.rs'");
        if phase == 2 {
            check(
                "document.querySelector('.maestro-message-error').textContent==='literal result'",
            );
        } else {
            check("document.querySelector('.maestro-code code').textContent==='literal code'");
        }
    }
    drop(row);
    released();
}
fn assistant(surface: &mut Surface, call: &ToolCall, result: &mut ToolResultMessage<Details>) {
    let message = serde_json::from_value(json!({"role":"assistant","content":[call],"api":"openai-completions","provider":"openai","model":"controlled","usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},"stopReason":"stop","timestamp":0})).unwrap();
    let host = surface.host();
    let mut row = AssistantMessage::new(host.clone());
    for (phase, expected) in [
        (0, "Creating artifact"),
        (1, "Created artifact"),
        (2, "Creating artifact"),
    ] {
        result.is_error = phase == 2;
        let results: HashMap<_, _> = if phase == 0 {
            HashMap::new()
        } else {
            [(call.id.clone(), result.clone())].into()
        };
        let mut draw = |ui: &mut egui::Ui,
                        frame: maestro_web::components::source::CodeBlockView<
            '_,
            maestro_web::TranslationFunction,
        >| {
            row.render(
                ui,
                &message,
                AssistantMessageProps {
                    tool_results_by_id: Some(&results),
                    is_streaming: phase == 0,
                    ..Default::default()
                },
                view(&host, frame),
            )
            .assert_no_overlays()
            .unwrap()
        };
        surface.draw(&mut draw);
        surface.draw(&mut draw);
        caption(expected);
        let _ = js("document.querySelector('.maestro-tool-caption').click()");
        surface.time = Some(surface.time.unwrap() + 0.3);
        surface.draw(&mut draw);
        surface.draw(&mut draw);
        check("document.querySelector('.maestro-artifact-pill').textContent==='literal.rs'");
        if phase == 2 {
            check(
                "document.querySelector('.maestro-message-error').textContent==='literal result'",
            );
        } else {
            check("document.querySelector('.maestro-code code').textContent==='literal code'");
        }
    }
    drop(row);
    released();
}
fn registry(
    surface: &mut Surface,
    call: &ToolCall,
    result: &ToolResultMessage<Details>,
    original: ToolRendererBinding,
) {
    let replacement: Rc<dyn ToolRenderer<JsonObject, Details>> =
        Rc::new(crate::recipes::Renderer(Rc::new(|_, _, _| {
            Ok(crate::recipes::content("replacement artifact", true))
        })));
    register_tool_renderer(
        "artifacts",
        ToolRendererBinding::from_raw(replacement.clone()),
    );
    assert!(Rc::ptr_eq(
        &replacement,
        &maestro_web::get_tool_renderer::<JsonObject, Details>("artifacts").unwrap()
    ));
    let mut row = ToolMessage::new();
    draw_tool(surface, &mut row, call, Some(result));
    assert!(
        surface
            .painted
            .iter()
            .any(|(text, _)| text == "replacement artifact")
    );
    set_show_json_mode(true);
    draw_tool(surface, &mut row, call, Some(result));
    check(
        "document.querySelector('.maestro-artifact-pill')===null && document.querySelector('.maestro-code')!==null",
    );
    set_show_json_mode(false);
    draw_tool(surface, &mut row, call, Some(result));
    assert!(
        surface
            .painted
            .iter()
            .any(|(text, _)| text == "replacement artifact")
    );
    drop(TOOL_RENDERERS.with(|all| all.borrow_mut().shift_remove("artifacts")));
    draw_tool(surface, &mut row, call, Some(result));
    check(
        "document.querySelector('.maestro-artifact-pill')===null && document.querySelector('.maestro-code')!==null",
    );
    register_tool_renderer("artifacts", original);
    draw_tool(surface, &mut row, call, Some(result));
    caption("Creating artifact");
    drop(row);
    released();
}
fn released() {
    check(
        "document.querySelectorAll('.maestro-tool-caption,.maestro-artifact-pill,.maestro-code,.maestro-console,.maestro-diff,.maestro-message-error').length===0",
    );
}

/// A rich SDK header without a pill keeps its original child and keyboard owner.
/// # Panics
/// Panics if a status change replaces its child, focus or disclosure state.
pub fn rich_header() {
    use maestro_web::{
        render_collapsible_header,
        tools::renderer_registry::{ToolHeaderState, ToolHeaderText},
    };
    let mut surface = Surface::new();
    surface.time = Some(0.);
    let expanded = Rc::new(Cell::new(false));
    let mut state = ToolRenderState::default();
    for status in [
        ToolHeaderState::Inprogress,
        ToolHeaderState::Complete,
        ToolHeaderState::Error,
    ] {
        let recipe = maestro_web::tools::types::ToolRenderResult {
            content: render_collapsible_header(
                status,
                maestro_web::icons::icons()
                    .unwrap()
                    .shift_remove("Code")
                    .unwrap(),
                ToolHeaderText::Content(Box::new(drawing::ListingRecipe {
                    code: "SDK rich".into(),
                })),
                expanded.clone(),
            ),
            is_custom: false,
        };
        drawing::draw(&mut surface, &recipe, &mut state);
        drawing::draw(&mut surface, &recipe, &mut state);
        if status == ToolHeaderState::Inprogress {
            let at = drawing::glyphs(&surface)[0].center();
            drawing::click(&mut surface, at, &recipe, &mut state);
            let _ = js("globalThis.richSDKCode=document.querySelector('.maestro-code')");
        }
        check("richSDKCode===document.querySelector('.maestro-code')");
        assert!(expanded.get());
        assert!(surface.context.memory(egui::Memory::focused).is_some());
        for (key, expected) in [(egui::Key::Space, false), (egui::Key::Enter, true)] {
            for pressed in [true, false] {
                surface.events = vec![egui::Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }];
                drawing::draw(&mut surface, &recipe, &mut state);
            }
            assert_eq!(expanded.get(), expected);
            check("richSDKCode===document.querySelector('.maestro-code')");
        }
    }
    drop(state);
    check("!richSDKCode.isConnected");
}

/// An artifact caption without a filename retains the shared native activation path.
/// # Panics
/// Panics if status-only updates replace its label, focus, selection or open state.
pub fn caption_without_pill() {
    let mut surface = Surface::new();
    surface.time = Some(0.);
    let params = json!({"command":"create","content":"literal code"});
    let renderer = ArtifactsToolRenderer::new(None);
    let mut state = ToolRenderState::default();
    for streaming in [true, false, true] {
        let recipe = <ArtifactsToolRenderer as ToolRenderer<Value, ()>>::render(
            &renderer,
            Some(&params),
            None,
            streaming,
        )
        .unwrap();
        drawing::draw(&mut surface, &recipe, &mut state);
        drawing::draw(&mut surface, &recipe, &mut state);
        if js("globalThis.noPillCaption===undefined")
            .as_bool()
            .unwrap()
        {
            let _ = js(
                "globalThis.noPillCaption=document.querySelector('.maestro-tool-caption');noPillCaption.click();const r=document.createRange();r.selectNodeContents(noPillCaption);getSelection().removeAllRanges();getSelection().addRange(r)",
            );
        }
        drawing::draw(&mut surface, &recipe, &mut state);
        surface.time = Some(surface.time.unwrap() + 0.3);
        let response = drawing::draw(&mut surface, &recipe, &mut state);
        check(
            "noPillCaption===document.querySelector('.maestro-tool-caption') && getSelection().toString()==='Creating artifact' && document.querySelector('.maestro-artifact-pill')===null",
        );
        assert!(response.rect.height() > 40.);
        assert!(surface.context.memory(egui::Memory::focused).is_some());
        for (key, open) in [(egui::Key::Space, false), (egui::Key::Enter, true)] {
            for pressed in [true, false] {
                surface.events = vec![egui::Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }];
                drawing::draw(&mut surface, &recipe, &mut state);
            }
            surface.time = Some(surface.time.unwrap() + 0.3);
            let response = drawing::draw(&mut surface, &recipe, &mut state);
            assert_eq!(response.rect.height() > 40., open);
        }
    }
    drop(state);
    check("!noPillCaption.isConnected");
    let _ = js("delete globalThis.noPillCaption");
}

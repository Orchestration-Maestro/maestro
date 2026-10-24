//! Opened attachments reach one containing receiver through every document path.
#![cfg(all(test, target_arch = "wasm32"))]
use super::{
    escape, file_subset,
    forwarding::{Header, TileRecipe, registration},
    prepare,
};
use crate::{
    markdown_support::{Surface, check},
    support::{reading_view::view, tool_output::TextOnlyOutput as _},
};
use maestro_web::{
    AssistantMessage, AssistantMessageProps, AttachmentOverlay, ExpandableSection, ToolMessage,
    ToolMessageProps, UserMessage, UserMessageInput, tools::types::ToolRenderState,
};
use std::{cell::Cell, rc::Rc};

fn label(surface: &Surface, name: &str) -> egui::Pos2 {
    surface
        .painted
        .iter()
        .find(|(text, _)| text == name)
        .expect("tile label is painted")
        .1
        .center()
}
fn preview(surface: &mut Surface, overlay: &mut AttachmentOverlay) -> bool {
    surface.draw(|ui, frame| {
        overlay
            .render(ui.ctx(), frame.i18n, frame.presentation)
            .unwrap()
    })
}
fn close(surface: &mut Surface, overlay: &mut AttachmentOverlay) {
    surface.events = vec![escape()];
    assert!(!preview(surface, overlay));
}
fn assistant() -> maestro_app::AssistantMessage {
    serde_json::from_value(serde_json::json!({"role":"assistant","content":[{"type":"toolCall","id":"call","name":"document-owner","arguments":{}}],"api":"openai-completions","provider":"openai","model":"controlled","usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},"stopReason":"stop","timestamp":0})).unwrap()
}

pub fn assistant_and_children() {
    for header in [Header::Plain, Header::Rich] {
        let fail = registration("document-owner", false, header);
        let mut surface = Surface::new();
        let host = surface.host();
        let mut row = AssistantMessage::new(host.clone());
        let message = assistant();
        surface.draw(|ui, frame| {
            row.render::<serde_json::Value>(
                ui,
                &message,
                AssistantMessageProps::default(),
                view(&host, frame),
            )
            .assert_no_overlays()
            .unwrap()
        });
        let at = label(&surface, "note.txt");
        fail.set(true);
        let mut receiver = Vec::new();
        surface.click(at, |ui, frame| {
            let output = row.render::<serde_json::Value>(ui, &message, AssistantMessageProps::default(), view(&host, frame));
            receiver.extend(output.overlays);
            assert!(matches!(output.response, Err(maestro_web::tools::types::ToolUiError::Presentation(ref message)) if message == "controlled later child failure"));
        });
        assert_eq!(receiver.len(), 1);
        drop(row);
        assert!(preview(&mut surface, &mut receiver[0]));
        check("document.body.textContent.includes('<literal>')");
        close(&mut surface, &mut receiver[0]);
    }
    generic_and_custom();
}
fn generic_and_custom() {
    let mut surface = Surface::new();
    let host = surface.host();
    let fail = Rc::new(Cell::new(false));
    let record = crate::attachment_support::attachment();
    let child = TileRecipe {
        record: record.clone(),
        fail: fail.clone(),
    };
    let mut section = ExpandableSection::new(host.clone(), vec![Box::new(child)]);
    section.default_expanded = true;
    surface.draw(|ui, frame| {
        section
            .render(ui, view(&host, frame))
            .assert_no_overlays()
            .unwrap()
    });
    let at = label(&surface, "note.txt");
    fail.set(true);
    let mut receiver = Vec::new();
    surface.click(at, |ui, frame| {
        let output = section.render(ui, view(&host, frame));
        receiver.extend(output.overlays);
        assert!(output.response.is_err());
    });
    assert_eq!(receiver.len(), 1);
    drop(section);
    assert!(preview(&mut surface, &mut receiver[0]));
    close(&mut surface, &mut receiver[0]);
    drop(receiver);
    custom(record);
}
fn custom(record: Rc<std::cell::RefCell<maestro_web::Attachment>>) {
    use crate::message_renderer_support::{Renderer, message};
    let mut surface = Surface::new();
    let host = surface.host();
    let fail = Rc::new(Cell::new(false));
    let captured = fail.clone();
    maestro_web::register_message_renderer(
        "document-custom",
        Rc::new(Renderer(Rc::new(move |_| {
            Ok(Box::new(TileRecipe {
                record: record.clone(),
                fail: captured.clone(),
            }))
        }))),
    );
    let input = message(
        "document-custom",
        std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0)),
    );
    let recipe = maestro_web::tools::types::ToolRenderResult {
        content: maestro_web::render_message(&input).unwrap().unwrap(),
        is_custom: true,
    };
    let mut state = ToolRenderState::default();
    surface.draw(|ui, frame| {
        recipe
            .render(ui, view(&host, frame), &mut state)
            .assert_no_overlays()
            .unwrap()
    });
    let at = label(&surface, "note.txt");
    fail.set(true);
    let mut receiver = Vec::new();
    surface.click(at, |ui, frame| {
        let output = recipe.render(ui, view(&host, frame), &mut state);
        receiver.extend(output.overlays);
        assert!(output.response.is_err());
    });
    assert_eq!(receiver.len(), 1);
    drop(state);
    assert!(preview(&mut surface, &mut receiver[0]));
    close(&mut surface, &mut receiver[0]);
}

pub fn returned_files() {
    super::browser::rows(|r| {
        r.group == "fileViews"
            || r.test
                .as_deref()
                .is_some_and(|name| name.starts_with("repl_downloads_"))
    });
    let row = file_subset(&[0, 1, 2]);
    let recipe = prepare(&row).unwrap();
    let mut surface = Surface::new();
    let host = surface.host();
    let mut state = ToolRenderState::default();
    surface.draw(|ui, frame| {
        recipe
            .render(ui, view(&host, frame), &mut state)
            .assert_no_overlays()
            .unwrap()
    });
    let image = crate::markdown_support::js(
        "(()=>{const r=document.querySelector('img[alt=\"image.png\"]').getBoundingClientRect();return [r.x+r.width/2,r.y+r.height/2]})()",
    );
    let image = js_sys::Float32Array::new(&image);
    let positions = [
        egui::pos2(image.get_index(0), image.get_index(1)),
        label(&surface, "雪.txt"),
        label(&surface, "a.json"),
    ];
    let mut receiver = Vec::new();
    for at in positions {
        surface.click(at, |ui, frame| {
            let output = recipe.render(ui, view(&host, frame), &mut state);
            receiver.extend(output.overlays);
            output.response.unwrap();
        });
    }
    assert_eq!(receiver.len(), 3);
    drop(state);
    assert!(preview(&mut surface, &mut receiver[0]));
    check("document.querySelector('img[alt=\"image.png\"]')!==null");
    assert!(preview(&mut surface, &mut receiver[1]));
    check("document.body.textContent.includes('雪')");
    assert!(preview(&mut surface, &mut receiver[2]));
    close(&mut surface, &mut receiver[2]);
    receiver.pop();
    assert!(preview(&mut surface, &mut receiver[1]));
    close(&mut surface, &mut receiver[1]);
    receiver.pop();
    assert!(preview(&mut surface, &mut receiver[0]));
    close(&mut surface, &mut receiver[0]);
}

pub fn user_and_tool() {
    let mut surface = Surface::new();
    let host = surface.host();
    let mut user = UserMessage::new(host.clone());
    let record = crate::attachment_support::attachment();
    record.borrow_mut().file_name = "user.txt".into();
    let records = [record];
    let content = maestro_app::UserContent::Text(String::new());
    let input = UserMessageInput::WithAttachments {
        content: &content,
        attachments: Some(&records),
    };
    let mut tool = ToolMessage::new();
    let call: maestro_app::ToolCall = serde_json::from_value(serde_json::json!({"type":"toolCall","id":"call","name":"javascript_repl","arguments":{"title":"Download","code":""}})).unwrap();
    let result = crate::recipes::message(Some(
        serde_json::json!({"files":[{"fileName":"tool.txt","mimeType":"text/plain","size":3,"contentBase64":"6Zuq"}]}),
    ));
    let props = ToolMessageProps {
        tool_call: &call,
        tool: None,
        result: Some(&result),
        pending: false,
        aborted: false,
        is_streaming: false,
    };
    let mut receiver = Vec::new();
    let mut draw = |ui: &mut egui::Ui,
                    frame: maestro_web::components::source::CodeBlockView<
        '_,
        maestro_web::TranslationFunction,
    >| {
        for output in [
            user.render(ui, input, view(&host, frame)),
            tool.render(ui, props, view(&host, frame)),
        ] {
            receiver.extend(output.overlays);
            output.response.unwrap();
        }
    };
    surface.draw(&mut draw);
    let positions = [label(&surface, "user.txt"), label(&surface, "tool.txt")];
    for at in positions {
        surface.click(at, &mut draw);
    }
    assert_eq!(receiver.len(), 2);
    drop(user);
    drop(tool);
    assert!(preview(&mut surface, &mut receiver[0]));
    assert!(preview(&mut surface, &mut receiver[1]));
    check("document.body.textContent.includes('雪')");
    close(&mut surface, &mut receiver[1]);
    receiver.pop();
    assert!(preview(&mut surface, &mut receiver[0]));
    check("document.body.textContent.includes('<literal>')");
    close(&mut surface, &mut receiver[0]);
}

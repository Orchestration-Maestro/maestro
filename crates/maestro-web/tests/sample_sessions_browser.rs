//! Supplied conversations replayed through existing message, card and artifact owners.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::{
    artifact_presentation_support::Recorder,
    markdown_support::{Surface, check, js, publish},
    support::{reading_view::view, tool_output::TextOnlyOutput as _},
};
use maestro_app::{
    AgentMessage, AssistantContent, ToolResultMessage,
    browser::{components::messages::CustomAgentMessages, tools::artifacts::ArtifactsPanel},
};
use maestro_web::{AssistantMessage, AssistantMessageProps, UserMessage, UserMessageInput};
use serde_json::{Value, json};
use std::{collections::HashMap, rc::Rc, sync::Arc};
use wasm_bindgen::prelude::*;

/// Existing owned conversation envelope.
type Message = AgentMessage<CustomAgentMessages>;

/// Replay the short HTML conversation without executing a recorded tool.
/// # Panics
/// Panics if rendering, canvas identity or a replay observation differs.
#[wasm_bindgen]
pub async fn simple_html_session_replays_existing_browser_owners() {
    let initial = Surface::new();
    let canvas = js("document.getElementById('canvas')");
    replay("simpleHtml", [1, 2, 1, 3, 1]).await;
    assert!(js_sys::Object::is(
        &canvas,
        &js("document.getElementById('canvas')")
    ));
    drop(initial);
}

/// Replay the long conversation, including failures, fallback tools and both HTML edits.
/// # Panics
/// Panics if a recorded message, card or reconstructed file differs.
#[wasm_bindgen]
pub async fn long_session_replays_existing_browser_owners() {
    replay("longSession", [21, 53, 32, 61, 32]).await;
}

/// Decode only the two historical fields on a test-owned message copy.
fn project(raw: &Value) -> Message {
    let mut projected = raw.clone();
    projected["timestamp"] = json!(0);
    if raw["role"] == "toolResult" {
        projected.as_object_mut().unwrap().shift_remove("output");
        projected["content"] = json!([{"type":"text","text":raw["output"]}]);
    }
    let decoded = serde_json::from_value(projected.clone()).unwrap();
    // Invert the projection to verify that every other supplied member survives.
    projected.as_object_mut().unwrap().shift_remove("timestamp");
    if raw["role"] == "toolResult" {
        projected.as_object_mut().unwrap().shift_remove("content");
        projected["output"] = raw["output"].clone();
    }
    assert_eq!(&projected, raw);
    let canonical = serde_json::to_value(&decoded).unwrap();
    assert_eq!(canonical["timestamp"].as_f64(), Some(0.0));
    assert_eq!(canonical["role"], raw["role"]);
    if raw["role"] == "toolResult" {
        assert_eq!(
            canonical["content"],
            json!([{"type":"text","text":raw["output"]}])
        );
        for key in ["toolCallId", "toolName", "isError", "details"] {
            assert_eq!(canonical[key], raw[key]);
        }
    } else {
        assert_eq!(canonical["content"], raw["content"]);
    }
    decoded
}

/// Load the complete supplied model, prompt and owned conversation operands.
fn load(name: &str) -> (Value, Vec<Message>) {
    let source: Value = serde_json::from_str(include_str!("fixtures/test_sessions.json")).unwrap();
    let session = &source[name];
    let model: maestro_app::Model = serde_json::from_value(session["model"].clone()).unwrap();
    assert_eq!(model.id, session["model"]["id"].as_str().unwrap());
    assert!(!session["systemPrompt"].as_str().unwrap().is_empty());
    let raw = session["messages"].as_array().unwrap();
    let messages: Vec<_> = raw.iter().map(project).collect();
    (session["messages"].clone(), messages)
}

/// Drive a complete recorded group through real owned message dispatch and reconstruction.
async fn replay(name: &str, counts: [usize; 5]) {
    crate::renderer_recipes::reset();
    maestro_web::register_tool_renderer(
        "artifacts",
        maestro_web::tools::types::ToolRendererBinding::from_raw::<maestro_app::JsonObject, Value>(
            Rc::new(maestro_web::ArtifactsToolRenderer::new(None)),
        ),
    );
    let (raw, messages) = load(name);
    let results: HashMap<String, ToolResultMessage> = messages
        .iter()
        .filter_map(|m| {
            if let Message::ToolResult(result) = m {
                let result = result.read().unwrap().clone();
                Some((result.tool_call_id.clone(), result))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(results.len(), counts[2]);
    let mut observed = [0; 5];
    let mut surface = Surface::new();
    surface.size = (1200., 1200.);
    for (index, message) in messages.iter().enumerate() {
        match message {
            Message::User(user) => {
                observed[0] += 1;
                observed[3] += raw[index]["content"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|c| c["type"] == "text")
                    .count();
                let user = user.read().unwrap().clone();
                draw_user(&mut surface, &user);
            }
            Message::Assistant(assistant) => {
                observed[1] += 1;
                let assistant = assistant.read().unwrap().clone();
                observed[3] += assistant
                    .content
                    .iter()
                    .filter(|c| matches!(c, AssistantContent::Text(_)))
                    .count();
                observed[4] += assistant
                    .content
                    .iter()
                    .filter(|c| matches!(c, AssistantContent::ToolCall(_)))
                    .count();
                draw_assistant(&mut surface, &assistant, &results).await;
            }
            Message::ToolResult(_) => observed[2] += 1,
            Message::Custom(_) => panic!("unexpected conversation role"),
        }
    }
    assert_eq!(observed, counts);
    reconstruct(name, &messages);
    crate::renderer_recipes::reset();
}

/// Mount and release one user row, observing its actual Markdown text.
fn draw_user(surface: &mut Surface, message: &maestro_app::UserMessage) {
    let mut row = UserMessage::new(surface.host());
    let host = surface.host();
    surface.draw(|ui, frame| {
        row.render(ui, UserMessageInput::User(message), view(&host, frame))
            .response
            .unwrap()
    });
    check("document.querySelector('.maestro-markdown').textContent.trim().length>0");
    if let maestro_app::UserContent::Blocks(blocks) = &message.content
        && let Some(maestro_app::UserBlock::Text(text)) = blocks.first()
        && matches!(text.text.as_str(), "minimal html" | "simple html")
    {
        publish("sampleUser", &text.text.as_str().into());
        check("document.querySelector('.maestro-markdown').textContent.trim()===sampleUser");
    }
    drop(row);
    check("document.querySelector('.maestro-markdown')===null");
}

/// Mount each assistant with its actual result map, then inspect expanded tool bodies.
async fn draw_assistant(
    surface: &mut Surface,
    message: &maestro_app::AssistantMessage,
    results: &HashMap<String, ToolResultMessage>,
) {
    let mut row = AssistantMessage::new(surface.host());
    let host = surface.host();
    let mut draw = |surface: &mut Surface| {
        surface.draw(|ui, frame| {
            row.render(
                ui,
                message,
                AssistantMessageProps {
                    tool_results_by_id: Some(results),
                    ..Default::default()
                },
                view(&host, frame),
            )
            .assert_no_overlays()
            .unwrap()
        });
    };
    draw(surface);
    let _ = js("document.querySelectorAll('.maestro-tool-caption').forEach(n=>n.click())");
    draw(surface);
    crate::markdown_support::settle().await;
    if message.stop_reason == maestro_app::StopReason::Error {
        publish(
            "sampleError",
            &format!("Error: {}", message.error_message.as_deref().unwrap()).into(),
        );
        check(
            "(()=>{const e=document.querySelector('.maestro-message-error');return e.textContent===sampleError && e.querySelector('strong').textContent==='Error:' && e.querySelector('span').childElementCount===0})()",
        );
    }
    if message.stop_reason == maestro_app::StopReason::Aborted {
        check("document.querySelector('.maestro-message-abort').textContent==='Request aborted'");
    }
    for chunk in &message.content {
        match chunk {
            AssistantContent::ToolCall(call) => observe_call(call, &results[&call.id]),
            AssistantContent::Text(text) if !text.text.trim().is_empty() => {
                observe_text(&text.text);
            }
            _ => {}
        }
    }
    drop(row);
    check(
        "document.querySelectorAll('.maestro-markdown,.maestro-code,.maestro-console,.maestro-diff,.maestro-message-error,.maestro-message-abort').length===0",
    );
}

/// Observe message text independently of the tool listings in the same row.
fn observe_text(text: &str) {
    check("document.querySelector('.maestro-markdown')!==null");
    for excerpt in [
        "I'll create a minimal HTML file",
        "I've created a minimal HTML file",
        "Third console log added!",
    ] {
        if text.contains(excerpt) {
            publish("sampleExcerpt", &excerpt.into());
            check(
                "[...document.querySelectorAll('.maestro-markdown')].some(n=>n.textContent.includes(sampleExcerpt))",
            );
        }
    }
}

/// Observe source-bearing artifact cards and the ordinary fallback's input/result listings.
fn observe_call(call: &maestro_app::ToolCall, result: &ToolResultMessage) {
    assert_eq!(call.id, result.tool_call_id);
    assert_eq!(call.name, result.tool_name);
    publish(
        "sampleArguments",
        &serde_json::to_string(&call.arguments).unwrap().into(),
    );
    let output = maestro_app::UserBlock::text_output(&result.content);
    publish("sampleOutput", &output.into());
    if call.name == "artifacts" {
        if call.arguments["command"] == "create" {
            publish(
                "sampleSource",
                &call.arguments["content"].as_str().unwrap().into(),
            );
            check(
                "[...document.querySelectorAll('.maestro-code code')].some(n=>n.textContent===sampleSource)",
            );
        } else {
            observe_update(call);
        }
        if maestro_app::browser::tools::artifacts::get_file_type(
            call.arguments["filename"].as_str().unwrap(),
        ) == maestro_app::browser::tools::artifacts::FileType::Html
        {
            check("document.querySelector('.maestro-console pre').textContent===sampleOutput");
        } else {
            check("document.querySelector('.maestro-console')===null");
        }
    } else {
        check(
            "[...document.querySelectorAll('.maestro-code code')].some(n=>{try{return JSON.stringify(JSON.parse(n.textContent))===JSON.stringify(JSON.parse(sampleArguments))}catch{return false}})",
        );
        check(
            "[...document.querySelectorAll('.maestro-code code')].some(n=>n.textContent===sampleOutput)",
        );
    }
}

/// Distinguish the added console line in each of the two recorded updates.
fn observe_update(call: &maestro_app::ToolCall) {
    let expected = if call.arguments["new_str"]
        .as_str()
        .unwrap()
        .contains("Third console log added!")
    {
        "console.log('Third console log added!');"
    } else {
        "console.log('Welcome to the simple HTML page');"
    };
    publish("sampleReplacement", &expected.into());
    check(
        "[...document.querySelectorAll('.maestro-diff-added')].some(n=>n.textContent.includes(sampleReplacement))",
    );
}

/// Compare the actual application reconstruction with independently recorded complete files.
fn reconstruct(name: &str, messages: &[Message]) {
    let expected: Value =
        serde_json::from_str(include_str!("fixtures/test_sessions.expected.json")).unwrap();
    let recorder = Arc::new(Recorder::default());
    let panel = ArtifactsPanel::new(recorder.clone());
    crate::artifact_support::presentation::ready(panel.reconstruct_from_messages(messages));
    let artifacts = panel.artifacts();
    let actual: Vec<Value> = artifacts.read().unwrap().iter().map(|(filename, artifact)| json!({"filename":filename,"content":artifact.read().unwrap().content})).collect();
    assert_eq!(json!(actual), expected[name]["files"]);
    assert_eq!(json!(panel.active_filename()), expected[name]["active"]);
    assert!(!recorder.take().is_empty());
    assert!(!recorder.providers.lock().unwrap().is_empty());
}

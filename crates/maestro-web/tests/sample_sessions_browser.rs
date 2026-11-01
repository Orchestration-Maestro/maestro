//! Supplied conversations replayed through the native message, card and artifact owners.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::{
    artifact_presentation_support::Recorder,
    contributions_support::{container, text, view},
};
use maestro_app::{
    AgentMessage, AssistantContent, ToolResultMessage,
    browser::{components::messages::CustomAgentMessages, tools::artifacts::ArtifactsPanel},
};
use maestro_web::{
    AssistantMessage, UserMessage,
    components::messages::ToolResults,
    dom::{DomContent as _, DomView},
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    rc::Rc,
    sync::{Arc, RwLock},
};
use wasm_bindgen::{JsCast as _, prelude::*};
use web_sys::{Element, HtmlElement};

/// Existing owned conversation envelope.
type Message = AgentMessage<CustomAgentMessages>;

/// Replay the short HTML conversation without executing a recorded tool.
/// # Panics
/// Panics if rendering or a replay observation differs.
#[wasm_bindgen]
pub fn simple_html_session_replays_existing_browser_owners() {
    replay("simpleHtml", [1, 2, 1, 3, 1]);
}

/// Replay the long conversation, including failures, fallback tools and both HTML edits.
/// # Panics
/// Panics if a recorded message, card or reconstructed file differs.
#[wasm_bindgen]
pub fn long_session_replays_existing_browser_owners() {
    replay("longSession", [21, 53, 32, 61, 32]);
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
fn replay(name: &str, counts: [usize; 5]) {
    crate::renderer_recipes::reset();
    maestro_web::register_tool_renderer(
        "artifacts",
        maestro_web::tools::types::ToolRendererBinding::from_raw::<maestro_app::JsonObject, Value>(
            Rc::new(maestro_web::ArtifactsToolRenderer::new(None)),
        ),
    );
    let (raw, messages) = load(name);
    let results: HashMap<String, Arc<RwLock<ToolResultMessage>>> = messages
        .iter()
        .filter_map(|m| match m {
            Message::ToolResult(result) => {
                let id = result.read().unwrap().tool_call_id.clone();
                Some((id, Arc::clone(result)))
            }
            _ => None,
        })
        .collect();
    assert_eq!(results.len(), counts[2]);
    let results = Rc::new(results);
    let view = view();
    let parent = container();
    let mut observed = [0; 5];
    for (index, message) in messages.iter().enumerate() {
        match message {
            Message::User(_) => {
                observed[0] += 1;
                observed[3] += raw[index]["content"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|c| c["type"] == "text")
                    .count();
                show_user(&parent, &view, message);
            }
            Message::Assistant(assistant) => {
                observed[1] += 1;
                let content = &assistant.read().unwrap().content;
                observed[3] += content
                    .iter()
                    .filter(|c| matches!(c, AssistantContent::Text(_)))
                    .count();
                observed[4] += content
                    .iter()
                    .filter(|c| matches!(c, AssistantContent::ToolCall(_)))
                    .count();
                show_assistant(&parent, &view, assistant, &results);
            }
            Message::ToolResult(_) => observed[2] += 1,
            Message::Custom(_) => panic!("unexpected conversation role"),
        }
    }
    assert_eq!(observed, counts);
    parent.remove();
    reconstruct(name, &messages);
    crate::renderer_recipes::reset();
}

/// Mount and release one user row, observing its actual Markdown text.
fn show_user(parent: &Element, view: &DomView, message: &Message) {
    let row = UserMessage {
        message: Some(message.clone()),
    };
    let root = row.mount(view).unwrap();
    parent.append_child(&root.node).unwrap();
    let markdown = text(&crate::contributions_support::select(
        &root.node,
        ".maestro-markdown",
    ));
    assert!(!markdown.trim().is_empty());
    if let Message::User(user) = message
        && let maestro_app::UserContent::Blocks(blocks) = &user.read().unwrap().content
        && let Some(maestro_app::UserBlock::Text(first)) = blocks.first()
        && matches!(first.text.as_str(), "minimal html" | "simple html")
    {
        assert_eq!(markdown.trim(), first.text);
    }
    root.node.unchecked_ref::<Element>().remove();
}

/// Mount each assistant with the actual result map, expand its cards and inspect their bodies.
fn show_assistant(
    parent: &Element,
    view: &DomView,
    message: &maestro_app::SharedAssistantMessage,
    results: &Rc<HashMap<String, Arc<RwLock<ToolResultMessage>>>>,
) {
    let row = AssistantMessage::<Value> {
        message: Some(Arc::clone(message)),
        tool_results_by_id: Some(ToolResults::Live(Rc::clone(results))),
        ..AssistantMessage::default()
    };
    let root = row.mount(view).unwrap();
    parent.append_child(&root.node).unwrap();
    let element: &Element = root.node.unchecked_ref();
    for toggle in all(element, ".maestro-tool-toggle") {
        toggle.unchecked_ref::<HtmlElement>().click();
    }
    let message = message.read().unwrap();
    if message.stop_reason == maestro_app::StopReason::Error {
        let error = crate::contributions_support::select(element, ".maestro-message-error");
        let expected = format!("Error: {}", message.error_message.as_deref().unwrap());
        assert_eq!(text(&error), expected);
        assert_eq!(
            text(&crate::contributions_support::select(&error, "strong")),
            "Error:"
        );
        assert_eq!(
            error.child_element_count(),
            1,
            "the message stays literal text"
        );
    }
    if message.stop_reason == maestro_app::StopReason::Aborted {
        let abort = crate::contributions_support::select(element, ".maestro-message-abort");
        assert_eq!(text(&abort), "Request aborted");
    }
    for chunk in &message.content {
        match chunk {
            AssistantContent::ToolCall(call) => {
                observe_call(element, call, &results[&call.id].read().unwrap());
            }
            AssistantContent::Text(text) if !text.text.trim().is_empty() => {
                observe_text(element, &text.text);
            }
            _ => {}
        }
    }
    element.remove();
}

/// Descendants of `root` matching `selector`, in document order.
fn all(root: &Element, selector: &str) -> Vec<Element> {
    let nodes = root.query_selector_all(selector).unwrap();
    (0..nodes.length())
        .map(|index| nodes.item(index).unwrap().unchecked_into())
        .collect()
}

/// Text of every match of `selector` under `root`.
fn texts(root: &Element, selector: &str) -> Vec<String> {
    all(root, selector).iter().map(|node| text(node)).collect()
}

/// Observe message text independently of the tool listings in the same row.
fn observe_text(row: &Element, text: &str) {
    let markdown = texts(row, ".maestro-markdown");
    assert!(!markdown.is_empty());
    for excerpt in [
        "I'll create a minimal HTML file",
        "I've created a minimal HTML file",
        "Third console log added!",
    ] {
        if text.contains(excerpt) {
            assert!(
                markdown.iter().any(|shown| shown.contains(excerpt)),
                "{excerpt}"
            );
        }
    }
}

/// Observe source-bearing artifact cards and the ordinary fallback's input/result listings.
fn observe_call(row: &Element, call: &maestro_app::ToolCall, result: &ToolResultMessage) {
    assert_eq!(call.id, result.tool_call_id);
    assert_eq!(call.name, result.tool_name);
    let output = maestro_app::UserBlock::text_output(&result.content);
    let listings = texts(row, ".maestro-code pre code");
    if call.name == "artifacts" {
        if call.arguments["command"] == "create" {
            let source = call.arguments["content"].as_str().unwrap();
            assert!(listings.iter().any(|listing| listing == source));
        } else {
            observe_update(row, call);
        }
        let consoles = texts(row, ".maestro-console pre");
        if maestro_app::browser::tools::artifacts::get_file_type(
            call.arguments["filename"].as_str().unwrap(),
        ) == maestro_app::browser::tools::artifacts::FileType::Html
        {
            assert_eq!(consoles.first(), Some(&output));
        } else {
            assert!(consoles.is_empty());
        }
    } else {
        assert!(listings.iter().any(|listing| {
            serde_json::from_str::<Value>(listing).ok().as_ref() == Some(&call.arguments)
        }));
        assert!(listings.contains(&output));
    }
}

/// Distinguish the added console line in each of the two recorded updates.
fn observe_update(row: &Element, call: &maestro_app::ToolCall) {
    let expected = if call.arguments["new_str"]
        .as_str()
        .unwrap()
        .contains("Third console log added!")
    {
        "console.log('Third console log added!');"
    } else {
        "console.log('Welcome to the simple HTML page');"
    };
    let added = texts(row, ".maestro-diff-added");
    assert!(
        added.iter().any(|line| line.contains(expected)),
        "{expected}"
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

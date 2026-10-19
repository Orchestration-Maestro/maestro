//! Closed real-Agent tool producer set and transcript event exclusion.
use super::{Model, frame, text};
use crate::{
    reading_slots_support::Slots,
    transcript_browser::aggregate::{Language, draw},
};
use maestro_app::{AgentEvent, AgentMessage, AgentToolRecord, AssistantContent, FauxResponseStep};
use maestro_web::{AgentInterface, I18n};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};
use wasm_bindgen::prelude::*;

/// Tool events cannot directly refresh even though their reduced state has already changed.
/// # Panics
/// Panics if a tool event refreshes the view or a later admitted event misses current results.
#[wasm_bindgen]
pub async fn session_tool_events_do_not_invent_transcript_updates() {
    let model = Model::new();
    let agent = model.session();
    let marker = initial_history(&agent);
    let scene = Rc::new(Slots::new());
    let language = Rc::new(I18n::new(Rc::new(Language)).unwrap());
    let view = Rc::new(RefCell::new(AgentInterface::new(scene.host.clone())));
    view.borrow_mut().set_session(Some(agent.clone())).unwrap();
    draw(&scene, &mut view.borrow_mut(), &language);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let (events, weak) = (seen.clone(), Rc::downgrade(&agent));
    let retained = view.clone();
    let subscription = agent.subscribe(Rc::new(move |event, _| {
        let agent = weak.upgrade().unwrap();
        let event = event.read().unwrap();
        let kind = tool_kind(&event);
        if let Some(kind) = kind {
            let before = text();
            marker.write().unwrap().content =
                maestro_app::UserContent::Text(format!("tool marker {kind}"));
            draw(&scene, &mut view.borrow_mut(), &language);
            assert_eq!(text(), before, "{kind} has no refresh branch");
            let pending = agent.state().read().unwrap().pending_tool_calls().to_vec();
            assert_eq!(pending.contains(&"controlled-call".into()), kind != "end");
            events.borrow_mut().push(kind);
        } else {
            draw(&scene, &mut view.borrow_mut(), &language);
        }
        Box::pin(std::future::ready(Ok(())))
    }));
    model.0.append_responses(responses());
    agent
        .prompt(maestro_app::PromptInput::Text {
            text: "run tools".into(),
            images: vec![],
        })
        .await
        .unwrap();
    subscription.unsubscribe();
    frame().await;
    assert!(retained.borrow().session().is_some());
    assert_eq!(&*seen.borrow(), &["start", "update", "end"]);
    assert!(text().contains("tool marker end"));
    assert!(text().contains("tool output"));
    assert!(text().contains("finished tools"));
}

/// The real executable tool reports one progress update before its result.
fn tool() -> AgentToolRecord {
    AgentToolRecord::<maestro_app::Threaded>::typed::<serde_json::Value, ()>(
        maestro_app::Tool {
            name: "controlled".into(),
            description: "Controlled result".into(),
            parameters: serde_json::json!({"type":"object"}),
        },
        "Controlled".into(),
        Rc::new(|_, _, _, update| {
            Box::pin(async move {
                let result = || maestro_app::AgentToolResult {
                    content: vec![maestro_app::UserBlock::Text(maestro_app::TextContent {
                        text: "tool output".into(),
                        text_signature: None,
                    })],
                    details: Some(()),
                    terminate: None,
                };
                update.unwrap()(result());
                Ok(result())
            })
        }),
    )
}

/// One tool turn followed by the normal assistant completion.
fn responses() -> Vec<FauxResponseStep> {
    let call = maestro_app::ToolCall {
        id: "controlled-call".into(),
        name: "controlled".into(),
        arguments: serde_json::Map::default(),
        thought_signature: None,
    };
    let mut response = maestro_app::faux_assistant_message(
        vec![AssistantContent::ToolCall(call)],
        maestro_app::FauxAssistantMessageOptions::default(),
    );
    response.stop_reason = maestro_app::StopReason::ToolUse;
    vec![
        FauxResponseStep::Message(Box::new(response)),
        FauxResponseStep::Message(Box::new(maestro_app::faux_assistant_message(
            "finished tools",
            maestro_app::FauxAssistantMessageOptions::default(),
        ))),
    ]
}

/// Enumerate the complete admitted producer set.
fn tool_kind(
    event: &AgentEvent<maestro_app::browser::components::messages::CustomAgentMessages>,
) -> Option<&'static str> {
    match event {
        AgentEvent::ToolExecutionStart { .. } => Some("start"),
        AgentEvent::ToolExecutionUpdate { .. } => Some("update"),
        AgentEvent::ToolExecutionEnd { .. } => Some("end"),
        AgentEvent::AgentStart
        | AgentEvent::AgentEnd { .. }
        | AgentEvent::TurnStart
        | AgentEvent::TurnEnd { .. }
        | AgentEvent::MessageStart { .. }
        | AgentEvent::MessageUpdate { .. }
        | AgentEvent::MessageEnd { .. } => None,
    }
}

/// Install the real executable declaration and a history marker before binding.
fn initial_history(agent: &super::Session) -> Arc<RwLock<maestro_app::UserMessage>> {
    let tool = tool();
    agent
        .state()
        .write()
        .unwrap()
        .set_tools(vec![Rc::new(RwLock::new(tool))]);
    let marker = Arc::new(RwLock::new(maestro_app::UserMessage {
        content: maestro_app::UserContent::Text("before tools".into()),
        timestamp: 0.,
    }));
    agent
        .state()
        .read()
        .unwrap()
        .messages()
        .write()
        .unwrap()
        .push(AgentMessage::User(marker.clone()));
    marker
}

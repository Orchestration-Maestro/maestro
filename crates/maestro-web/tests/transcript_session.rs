//! Session binding through the real browser Agent and its scripted model.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::{
    reading_slots_support::{Slots, js},
    transcript_browser::aggregate::{Language, draw},
};
use maestro_app::browser::components::messages::CustomAgentMessages as BrowserMessages;
use maestro_app::{
    Agent, AgentEvent, AgentOptions, FauxResponseStep, NoCustomMessages, PromptInput,
};
use maestro_web::{AgentInterface, I18n};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::prelude::*;

#[path = "transcript_session/lifecycle.rs"]
mod lifecycle;
#[path = "transcript_session/publication.rs"]
mod publication;
#[path = "transcript_session/requests.rs"]
mod requests;
#[path = "transcript_session/retry.rs"]
mod retry;
#[path = "transcript_session/tools.rs"]
mod tools;

/// Real execution family used by the public browser view.
type Session = Agent<BrowserMessages<NoCustomMessages>>;
/// A scripted provider unregisters even after its owning scene is retired.
struct Model(maestro_app::FauxProviderRegistration);
impl Model {
    /// Queue independent owned messages through the existing scripted provider.
    fn new() -> Self {
        Self(maestro_app::register_faux_provider(
            maestro_app::RegisterFauxProviderOptions::default(),
        ))
    }
    /// Construct the real Agent with deterministic callbacks and browser submission.
    fn session(&self) -> Rc<Session> {
        let agent = Rc::new(Agent::new(AgentOptions {
            spawn: Some(Rc::new(wasm_bindgen_futures::spawn_local)),
            stream_fn: Some(maestro_web::create_stream_fn(Rc::new(|| {
                Box::pin(async { Ok(None) })
            }))),
            get_api_key: Some(Rc::new(|_| {
                Box::pin(async { Ok(Some("controlled-key".into())) })
            })),
            ..Default::default()
        }));
        *agent.state().read().unwrap().model.write().unwrap() =
            self.0.get_model(None).unwrap().clone();
        agent
    }
    /// Each request consumes a real response instead of calling view observers directly.
    async fn prompt(&self, agent: &Rc<Session>, text: &str) {
        self.0
            .append_responses(vec![FauxResponseStep::Message(Box::new(
                maestro_app::faux_assistant_message(
                    text,
                    maestro_app::FauxAssistantMessageOptions::default(),
                ),
            ))]);
        agent
            .prompt(PromptInput::Text {
                text: text.into(),
                images: vec![],
            })
            .await
            .unwrap();
    }
}
impl Drop for Model {
    fn drop(&mut self) {
        self.0.unregister();
    }
}
/// Read the actual containing native text without retaining child owners.
fn text() -> String {
    js(
        &JsValue::NULL,
        "return document.querySelector('.maestro-transcript')?.innerText ?? ''",
    )
    .as_string()
    .unwrap()
}

/// Binding before and after connection preserves supplied callbacks and retires the previous session.
/// # Panics
/// Panics if defaults replace caller callbacks or retired sessions still refresh the view.
#[wasm_bindgen]
pub async fn session_rebinding_releases_old_subscription_and_defaults_only_missing_operations() {
    let model = Model::new();
    let scene = Slots::new();
    let i18n = I18n::new(Rc::new(Language)).unwrap();
    let first = model.session();
    let stream = first.stream_fn.read().unwrap().clone().unwrap();
    let key = first.get_api_key.read().unwrap().clone().unwrap();
    let mut interface = AgentInterface::new(scene.host.clone());
    assert!(interface.session().is_none());
    interface.set_session(Some(first.clone())).unwrap();
    assert!(Rc::ptr_eq(interface.session().unwrap(), &first));
    assert!(Rc::ptr_eq(
        &stream,
        first.stream_fn.read().unwrap().as_ref().unwrap()
    ));
    assert!(Rc::ptr_eq(
        &key,
        first.get_api_key.read().unwrap().as_ref().unwrap()
    ));
    model.prompt(&first, "before connection").await;
    draw(&scene, &mut interface, &i18n);
    assert!(text().contains("before connection"));
    let second = model.session();
    *second.get_api_key.write().unwrap() = None;
    let supplied = second.stream_fn.read().unwrap().clone().unwrap();
    interface.set_session(Some(second.clone())).unwrap();
    assert!(second.get_api_key.read().unwrap().is_some());
    assert!(Rc::ptr_eq(
        &supplied,
        second.stream_fn.read().unwrap().as_ref().unwrap()
    ));
    draw(&scene, &mut interface, &i18n);
    model.prompt(&first, "retired session").await;
    draw(&scene, &mut interface, &i18n);
    assert!(!text().contains("retired session"));
    *second.get_api_key.write().unwrap() = Some(key.clone());
    model.prompt(&second, "after connection").await;
    draw(&scene, &mut interface, &i18n);
    assert!(text().contains("after connection"));
    let third = model.session();
    *third.stream_fn.write().unwrap() = None;
    let supplied_key = third.get_api_key.read().unwrap().clone().unwrap();
    interface.set_session(Some(third.clone())).unwrap();
    assert!(third.stream_fn.read().unwrap().is_some());
    assert!(Rc::ptr_eq(
        &supplied_key,
        third.get_api_key.read().unwrap().as_ref().unwrap()
    ));
    interface.set_session(None).unwrap();
    assert!(interface.session().is_none());
    model.prompt(&second, "unbound session").await;
    draw(&scene, &mut interface, &i18n);
    assert!(text().is_empty());
}

/// Observe every producer event after the view's observer and render through the public aggregate.
/// # Panics
/// Panics if reduction order, completed history identity or streaming retirement differs.
#[wasm_bindgen]
pub async fn session_event_branches_publish_history_and_clear_streaming_once() {
    let model = Model::new();
    let scene = Rc::new(Slots::new());
    let i18n = Rc::new(I18n::new(Rc::new(Language)).unwrap());
    let agent = model.session();
    let history = agent.state().read().unwrap().messages().clone();
    let interface = Rc::new(RefCell::new(AgentInterface::new(scene.host.clone())));
    interface
        .borrow_mut()
        .set_session(Some(agent.clone()))
        .unwrap();
    let marker = std::sync::Arc::new(std::sync::RwLock::new(maestro_app::UserMessage {
        content: maestro_app::UserContent::Text("initial history".into()),
        timestamp: 0.,
    }));
    history
        .write()
        .unwrap()
        .push(maestro_app::AgentMessage::User(marker.clone()));
    draw(&scene, &mut interface.borrow_mut(), &i18n);
    let observed = Rc::new(RefCell::new(Vec::new()));
    let subscription = observe_events(
        &agent,
        (scene, i18n, interface.clone()),
        marker,
        observed.clone(),
    );
    model.prompt(&agent, "completed history once").await;
    subscription.unsubscribe();
    assert!(std::sync::Arc::ptr_eq(
        &history,
        agent.state().read().unwrap().messages()
    ));
    assert_completed(&observed.borrow());
}

/// Wait for native frame delivery without imposing a delay.
async fn frame() {
    let promise = js(
        &JsValue::NULL,
        "return new Promise(resolve=>requestAnimationFrame(resolve))",
    );
    wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(promise))
        .await
        .unwrap();
}
/// Count the actual retained native rows.
fn slots() -> usize {
    js(
        &JsValue::NULL,
        "return document.querySelectorAll('.maestro-transcript-slot').length.toString()",
    )
    .as_string()
    .unwrap()
    .parse()
    .unwrap()
}
/// Runtime reduction releases guards and publishes authoritative state before observation.
fn assert_reduced(agent: &Session, event: &AgentEvent<BrowserMessages<NoCustomMessages>>) {
    let state = agent.state().read().unwrap();
    assert!(state.is_streaming());
    match event {
        AgentEvent::MessageStart { .. } | AgentEvent::MessageUpdate { .. } => {
            assert!(state.streaming_message().is_some());
        }
        AgentEvent::MessageEnd { message } => {
            assert!(state.streaming_message().is_none());
            let history = state.messages().read().unwrap();
            match (history.last().unwrap(), message) {
                (maestro_app::AgentMessage::User(left), maestro_app::AgentMessage::User(right)) => {
                    assert!(std::sync::Arc::ptr_eq(left, right));
                }
                (
                    maestro_app::AgentMessage::Assistant(left),
                    maestro_app::AgentMessage::Assistant(right),
                ) => assert!(std::sync::Arc::ptr_eq(left, right)),
                _ => panic!("completed entry must be the reduced message"),
            }
        }
        AgentEvent::AgentEnd { .. } => assert!(state.streaming_message().is_none()),
        _ => {}
    }
}

/// Close the admitted producer set explicitly instead of classifying unknown events as refreshes.
fn event_name(event: &AgentEvent<BrowserMessages<NoCustomMessages>>) -> &'static str {
    match event {
        AgentEvent::AgentStart => "agent_start",
        AgentEvent::TurnStart => "turn_start",
        AgentEvent::MessageStart { .. } => "message_start",
        AgentEvent::MessageUpdate { .. } => "message_update",
        AgentEvent::MessageEnd { .. } => "message_end",
        AgentEvent::TurnEnd { .. } => "turn_end",
        AgentEvent::AgentEnd { .. } => "agent_end",
        _ => unreachable!(),
    }
}

/// Every admitted branch ran and the completed message appears exactly once.
fn assert_completed(events: &[(&str, String)]) {
    for name in [
        "agent_start",
        "turn_start",
        "message_start",
        "message_update",
        "message_end",
        "turn_end",
        "agent_end",
    ] {
        assert!(events.iter().any(|(event, _)| *event == name), "{name}");
    }
    let final_text = &events.last().unwrap().1;
    assert_eq!(
        final_text.matches("completed history once").count(),
        2,
        "one user and one completed assistant, no streaming duplicate"
    );
    assert!(!final_text.contains("Working"));
}

/// Recorded native text after each admitted runtime event.
type Events = Rc<RefCell<Vec<(&'static str, String)>>>;
/// Observe from after the view subscription, releasing every runtime guard before drawing.
fn observe_events(
    agent: &Rc<Session>,
    (canvas, language, view): (Rc<Slots>, Rc<I18n>, Rc<RefCell<AgentInterface>>),
    marker: std::sync::Arc<std::sync::RwLock<maestro_app::UserMessage>>,
    events: Events,
) -> maestro_app::Subscription<BrowserMessages<NoCustomMessages>> {
    let weak = Rc::downgrade(agent);
    agent.subscribe(Rc::new(move |event, _| {
        let agent = weak.upgrade().unwrap();
        let name = event_name(&event.read().unwrap());
        assert_reduced(&agent, &event.read().unwrap());
        assert!(agent.state().try_write().is_ok());
        marker.write().unwrap().content =
            maestro_app::UserContent::Text(format!("refreshed {name}"));
        draw(&canvas, &mut view.borrow_mut(), &language);
        assert_event_frame(&agent, name, &events.borrow());
        let partial = partial_text(&event.read().unwrap());
        let (canvas, language, view, events) = (
            canvas.clone(),
            language.clone(),
            view.clone(),
            events.clone(),
        );
        Box::pin(async move {
            if name == "message_update" {
                frame().await;
                draw(&canvas, &mut view.borrow_mut(), &language);
                assert!(
                    js(
                        &JsValue::NULL,
                        "return document.querySelectorAll('.maestro-transcript-slot').length"
                    )
                    .as_f64()
                    .unwrap()
                        >= 3.
                );
                assert!(
                    js(&JsValue::NULL, "return document.querySelector('.maestro-transcript-slot:last-child').innerText")
                        .as_string().unwrap().contains(partial.trim()),
                    "published partial: {partial:?}"
                );
            }
            events.borrow_mut().push((name, text()));
            Ok(())
        })
    }))
}

/// Every admitted event refreshes history; completed rows clear only the stream.
fn assert_event_frame(agent: &Session, name: &str, events: &[(&str, String)]) {
    assert!(
        text().contains(&format!("refreshed {name}")),
        "{name} must refresh history"
    );
    if name == "agent_end" {
        assert_eq!(slots(), 3, "completed history only, no stream slot");
    }
    if name == "message_end" {
        assert_eq!(
            slots(),
            agent
                .state()
                .read()
                .unwrap()
                .messages()
                .read()
                .unwrap()
                .len()
                + 1
        );
        assert_eq!(
            js(
                &JsValue::NULL,
                "return document.querySelectorAll('.maestro-transcript-cursor').length"
            )
            .as_f64(),
            Some(1.)
        );
    }
    let first_update =
        name == "message_update" && !events.iter().any(|(name, _)| *name == "message_update");
    if first_update {
        assert_eq!(
            text().matches("completed history once").count(),
            1,
            "stream is deferred, only user is published"
        );
    }
}

/// Read the actual partial text without assuming the producer emits a nonempty first update.
fn partial_text(event: &AgentEvent<BrowserMessages<NoCustomMessages>>) -> String {
    match event {
        AgentEvent::MessageUpdate { message, .. } => message
            .read()
            .unwrap()
            .content
            .iter()
            .filter_map(|content| {
                if let maestro_app::AssistantContent::Text(text) = content {
                    Some(text.text.as_str())
                } else {
                    None
                }
            })
            .collect::<String>(),
        _ => String::new(),
    }
}

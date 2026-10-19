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
    let observed = Rc::new(RefCell::new(Vec::new()));
    let events = observed.clone();
    let weak = Rc::downgrade(&agent);
    let view = interface.clone();
    let (canvas, language) = (scene.clone(), i18n.clone());
    let subscription = agent.subscribe(Rc::new(move |event, _| {
        let agent = weak.upgrade().unwrap();
        let state = agent.state().read().unwrap();
        let name = event_name(&event.read().unwrap());
        assert!(state.is_streaming());
        drop(state);
        assert!(agent.state().try_write().is_ok());
        draw(&canvas, &mut view.borrow_mut(), &language);
        if name == "agent_end" {
            assert_eq!(
                js(
                    &JsValue::NULL,
                    "return document.querySelectorAll('.maestro-transcript-slot').length"
                )
                .as_f64(),
                Some(2.)
            );
        }
        events.borrow_mut().push((name, text()));
        Box::pin(std::future::ready(Ok(())))
    }));
    model.prompt(&agent, "completed history once").await;
    subscription.unsubscribe();
    assert!(std::sync::Arc::ptr_eq(
        &history,
        agent.state().read().unwrap().messages()
    ));
    assert_completed(&observed.borrow());
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

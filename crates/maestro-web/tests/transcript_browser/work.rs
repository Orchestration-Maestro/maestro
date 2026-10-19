//! Actual aggregate work counts paired with the result-index owner's private controls.
use super::aggregate::{
    Language, Renderer, begin_slot_writes, draw, end_slot_writes, transcript_root,
};
use super::{check, js};
use crate::reading_slots_support::Slots;
use maestro_app::browser::components::messages::CustomAgentMessages as BrowserMessages;
use maestro_app::{Agent, AgentMessage, NoCustomMessages, UserContent, UserMessage};
use maestro_web::{
    AgentInterface, I18n, MessageRenderer,
    tools::types::{ToolContent, ToolUiError},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{Arc, RwLock},
};
use wasm_bindgen::prelude::*;

/// Count preparation separately from actual canvas/native recipe invocations.
struct Counted {
    prepared: Rc<Cell<usize>>,
    renderer: Renderer,
}
impl MessageRenderer<BrowserMessages<NoCustomMessages>> for Counted {
    fn render(
        &self,
        message: &AgentMessage<BrowserMessages<NoCustomMessages>>,
    ) -> Result<Box<dyn ToolContent>, ToolUiError> {
        self.prepared.set(self.prepared.get() + 1);
        self.renderer.render(message)
    }
}
/// Every emitted row has an interleaved suppressed result, exercising the real shared-index owner.
fn messages(start: usize, count: usize) -> Vec<AgentMessage<BrowserMessages<NoCustomMessages>>> {
    (start..start + count)
        .flat_map(|n| {
            [
                AgentMessage::User(Arc::new(RwLock::new(UserMessage {
                    content: UserContent::Text(format!("work row {n}")),
                    timestamp: 0.,
                }))),
                AgentMessage::ToolResult(Arc::new(RwLock::new(crate::renderer_recipes::message(
                    None,
                )))),
            ]
        })
        .collect()
}
/// Observe slot factories separately from the persistent native sizing publication.
fn mounted() -> usize {
    js(&JsValue::NULL, "return slotMounts.toString()")
        .as_string()
        .unwrap()
        .parse()
        .unwrap()
}
/// The counted operands and real view share one browser scene.
struct Work {
    scene: Slots,
    i18n: I18n,
    prepared: Rc<Cell<usize>>,
    calls: Rc<RefCell<Vec<String>>>,
    agent: Rc<Agent<BrowserMessages<NoCustomMessages>>>,
    interface: AgentInterface,
}
impl Work {
    /// Register a counted ordinary renderer, never a production instrumentation hook.
    fn new(count: usize) -> Self {
        let scene = Slots::new();
        let i18n = I18n::new(Rc::new(Language)).unwrap();
        let prepared = Rc::new(Cell::new(0));
        let calls = Rc::new(RefCell::new(vec![]));
        maestro_web::register_message_renderer(
            "user",
            Rc::new(Counted {
                prepared: prepared.clone(),
                renderer: Renderer(calls.clone()),
            }),
        );
        let agent = Rc::new(Agent::<BrowserMessages<NoCustomMessages>>::default());
        agent
            .state()
            .write()
            .unwrap()
            .set_messages(messages(0, count));
        let mut interface = AgentInterface::new(scene.host.clone());
        interface.set_auto_scroll(false);
        interface.set_session(Some(agent.clone())).unwrap();
        Self {
            scene,
            i18n,
            prepared,
            calls,
            agent,
            interface,
        }
    }
    /// Every dirty row is prepared and drawn once, with only new positional factories.
    fn dirty(&mut self, count: usize, mounts: usize) {
        begin_slot_writes();
        draw(&self.scene, &mut self.interface, &self.i18n);
        assert_eq!(end_slot_writes(), count);
        assert_eq!(mounted(), mounts);
        assert_eq!(self.prepared.replace(0), count);
        assert_eq!(self.calls.take().len(), count);
    }
    /// Clean frames draw current visible content with no preparation, factory work or slot sizing.
    fn clean(&mut self, expected: &str, count: usize) {
        begin_slot_writes();
        draw(&self.scene, &mut self.interface, &self.i18n);
        assert_eq!(end_slot_writes(), 0);
        assert_eq!(mounted(), 0);
        assert_eq!(self.prepared.replace(0), 0);
        let drawn = self.calls.take();
        assert!(
            drawn.len() < count && drawn.iter().any(|text| text == expected),
            "{drawn:?}"
        );
    }
    /// A newly admitted refresh must reconcile an offscreen edit without replacing positional slots.
    fn refresh_offscreen(&mut self, count: usize) {
        let history = self.agent.state().read().unwrap().messages().clone();
        let AgentMessage::User(last) = history.read().unwrap()[2 * (count - 1)].clone() else {
            unreachable!()
        };
        last.write().unwrap().content = UserContent::Text("updated offscreen native text".into());
        self.interface
            .set_session(Some(self.agent.clone()))
            .unwrap();
        self.dirty(count, 0);
        check(
            &transcript_root(),
            "return value.innerText.includes('updated offscreen native text') && [...value.querySelectorAll('.maestro-transcript-slot')].every(n=>n.isConnected)",
        );
    }
    /// Retain old slot identities while adding exactly the appended positions.
    fn extend(&mut self, count: usize) {
        let stable = js(
            &transcript_root(),
            "return [...value.querySelectorAll('.maestro-transcript-slot')]",
        );
        self.agent
            .state()
            .read()
            .unwrap()
            .messages()
            .write()
            .unwrap()
            .extend(messages(count, count));
        self.interface
            .set_session(Some(self.agent.clone()))
            .unwrap();
        self.dirty(2 * count, count);
        check(
            &stable,
            "const now=[...document.querySelectorAll('.maestro-transcript-slot')];return value.every((node,i)=>node===now[i])",
        );
    }
}
/// Initial and refreshed work scale with input; clean scroll draws only visible rows without stale native text.
/// # Panics
/// Panics if preparation, drawing, adapter sizing or positional extension rescans or recreates rows.
#[wasm_bindgen]
pub fn transcript_render_work_stays_linear_and_clean_frames_visit_only_visible_rows() {
    for count in [20, 40] {
        let mut work = Work::new(count);
        work.dirty(count, count);
        work.clean("work row 0", count);
        work.refresh_offscreen(count);
        work.clean("work row 0", count);
        bottom();
        work.clean("updated offscreen native text", count);
        work.extend(count);
        bottom();
        work.clean(&format!("work row {}", 2 * count - 1), 2 * count);
    }
}
/// Change the real viewport position without invalidating native row content.
fn bottom() {
    js(
        &transcript_root(),
        "value.parentElement.scrollTop=value.parentElement.scrollHeight",
    );
}

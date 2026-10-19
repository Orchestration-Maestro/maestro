//! Reached preparation failure must not consume a session refresh observation.
use super::{Model, text};
use crate::{
    reading_slots_support::{Slots, js, snapshot, unchanged},
    transcript_browser::aggregate::{Language, Renderer, draw, try_draw},
};
use maestro_app::browser::components::messages::CustomAgentMessages as BrowserMessages;
use maestro_app::{AgentMessage, NoCustomMessages};
use maestro_web::{
    AgentInterface, I18n, MessageRenderer,
    tools::types::{ToolContent, ToolUiError},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{JsCast as _, prelude::*};

/// A caller renderer can recover after a reached preparation failure.
struct Recovering {
    /// Failure belongs to this renderer rather than an invented host protocol.
    fail: Rc<Cell<bool>>,
    /// Actual preparation invocations at the refresh owner boundary.
    attempts: Rc<Cell<usize>>,
    /// Existing native reading recipe owner.
    renderer: Renderer,
}
impl MessageRenderer<BrowserMessages<NoCustomMessages>> for Recovering {
    fn render(
        &self,
        message: &AgentMessage<BrowserMessages<NoCustomMessages>>,
    ) -> Result<Box<dyn ToolContent>, ToolUiError> {
        self.attempts.set(self.attempts.get() + 1);
        if self.fail.get() {
            return Err(ToolUiError::Presentation(
                "controlled preparation failure".into(),
            ));
        }
        self.renderer.render(message)
    }
}
/// A failed refresh preserves prior native owners and retries the same authoritative history.
/// # Panics
/// Panics if a recoverable renderer failure consumes history invalidation.
#[wasm_bindgen]
pub async fn transcript_refresh_retries_failed_recipe_preparation() {
    let model = Model::new();
    let agent = model.session();
    let scene = Slots::new();
    let language = I18n::new(Rc::new(Language)).unwrap();
    let fail = Rc::new(Cell::new(false));
    let attempts = Rc::new(Cell::new(0));
    maestro_web::register_message_renderer(
        "user",
        Rc::new(Recovering {
            fail: fail.clone(),
            attempts: attempts.clone(),
            renderer: Renderer(Rc::new(RefCell::new(vec![]))),
        }),
    );
    model.prompt(&agent, "before failure").await;
    let mut view = AgentInterface::new(scene.host.clone());
    view.set_auto_scroll(false);
    view.set_session(Some(agent.clone())).unwrap();
    draw(&scene, &mut view, &language);
    let root: web_sys::Element = js(
        &JsValue::NULL,
        "return document.querySelector('.maestro-transcript')",
    )
    .dyn_into()
    .unwrap();
    js(
        &root,
        "const range=document.createRange();range.selectNodeContents(value.querySelector('p'));getSelection().removeAllRanges();getSelection().addRange(range)",
    );
    let saved = snapshot(&root);
    model.prompt(&agent, "after failure").await;
    fail.set(true);
    assert!(try_draw(&scene, &mut view, &language).is_err());
    unchanged(&saved);
    fail.set(false);
    let before = attempts.get();
    draw(&scene, &mut view, &language);
    assert!(
        attempts.get() > before,
        "failed refresh must reenter preparation"
    );
    assert!(
        text().contains("after failure"),
        "unacknowledged refresh must retry"
    );
    assert!(
        js(
            &root,
            "return value.isConnected && getSelection().toString()==='before failure'"
        )
        .as_bool()
        .unwrap()
    );
}

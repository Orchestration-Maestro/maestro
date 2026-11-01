//! Native resources retained by output recipes after their borrowing call ends.
#![cfg(test)]
use crate::contributions_support::{container, text, view};
use maestro_web::{
    dom::{DomContent, DomRoot, DomView, MountedContent},
    tools::types::{ToolRenderResult, ToolResultMessageView, ToolUiError},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{JsCast as _, closure::Closure, prelude::wasm_bindgen};

/// Original callback data plus state belonging to one placement.
struct Row {
    /// Activations received by this placement.
    clicks: Cell<usize>,
    /// The callback's original detail owner.
    original: Rc<Cell<i32>>,
}
/// The prepared caption and original native detail owner.
struct Action {
    /// Caption copied out of the transient argument borrow.
    label: String,
    /// Native details retained without serialization.
    original: Rc<Cell<i32>>,
    /// Placement state retained explicitly by the observing caller.
    aliases: Rc<RefCell<Vec<Rc<Row>>>>,
}
impl DomContent for Action {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let button = view.document.create_element("button").unwrap();
        button.set_text_content(Some(&format!("{} 0", self.label)));
        let row = Rc::new(Row {
            clicks: Cell::new(0),
            original: self.original.clone(),
        });
        self.aliases.borrow_mut().push(row.clone());
        let (state, label) = (row, self.label.clone());
        let target = js_sys::WeakRef::new(&button);
        let callback = Closure::<dyn Fn(web_sys::Event)>::new(move |_| {
            state.clicks.set(state.clicks.get() + 1);
            state.original.set(state.original.get() + 1);
            if let Some(button) = target.deref() {
                button.set_text_content(Some(&format!("{} {}", label, state.clicks.get())));
            }
        });
        button
            .add_event_listener_with_callback("click", callback.as_ref().unchecked_ref())
            .unwrap();
        let listener = Listener {
            target: js_sys::WeakRef::new(&button),
            callback,
        };
        Ok(DomRoot {
            node: button.into(),
            owner: Some(Box::new(listener)),
        })
    }
    fn update(&self, _: &DomRoot, _: &DomView) -> Result<(), ToolUiError> {
        Ok(())
    }
}
/// The test child's native callback, released with its placed root.
struct Listener {
    /// Non-owning route to the child's event target.
    target: js_sys::WeakRef<web_sys::Element>,
    /// Exact registration removed on release.
    callback: Closure<dyn Fn(web_sys::Event)>,
}
impl Drop for Listener {
    fn drop(&mut self) {
        if let Some(target) = self.target.deref() {
            target
                .remove_event_listener_with_callback(
                    "click",
                    self.callback.as_ref().unchecked_ref(),
                )
                .unwrap();
        }
    }
}
/// Typed callback preparing a child with the original native details.
struct Renderer(Rc<RefCell<Vec<Rc<Row>>>>);
impl maestro_web::ToolRenderer<String, Rc<Cell<i32>>> for Renderer {
    fn render(
        &self,
        params: Option<&String>,
        result: Option<ToolResultMessageView<'_, Rc<Cell<i32>>>>,
        _: bool,
    ) -> Result<ToolRenderResult, ToolUiError> {
        Ok(ToolRenderResult {
            content: Box::new(Action {
                label: params.unwrap().clone(),
                original: result.unwrap().details().unwrap().clone(),
                aliases: self.0.clone(),
            }),
            is_custom: true,
        })
    }
}
/// Each placement retains its own live action and can independently release its detail owner.
#[wasm_bindgen]
pub fn tool_content_keeps_owned_inputs_and_independent_row_lifetimes() {
    let aliases = Rc::new(RefCell::new(Vec::new()));
    let (prepared, weak) = prepare_owned(aliases.clone());
    let view = view();
    let parent = container();
    let mut first = MountedContent::default();
    first
        .show(prepared.content.as_ref(), &parent, &view)
        .unwrap();
    first
        .root()
        .unwrap()
        .unchecked_ref::<web_sys::HtmlElement>()
        .click();
    assert_eq!(weak.upgrade().unwrap().get(), 43);
    let mut second = MountedContent::default();
    second
        .show(prepared.content.as_ref(), &parent, &view)
        .unwrap();
    assert_eq!(text(second.root().unwrap()), "owned caption 0");
    first
        .show(prepared.content.as_ref(), &parent, &view)
        .unwrap();
    assert_eq!(text(first.root().unwrap()), "owned caption 1");
    let button = first.root().unwrap().clone();
    first.clear();
    assert!(
        second.root().unwrap().is_connected(),
        "the other placement stays live"
    );
    second.clear();
    drop(prepared);
    super::reset();
    let [first_alias, second_alias]: [_; 2] = aliases.take().try_into().ok().unwrap();
    assert_eq!(first_alias.clicks.get(), 1);
    assert_eq!(second_alias.clicks.get(), 0);
    button.unchecked_ref::<web_sys::HtmlElement>().click();
    assert_eq!(
        first_alias.clicks.get(),
        1,
        "released root loses its listener"
    );
    first_alias.original.set(44);
    assert_eq!(weak.upgrade().unwrap().get(), 44);
    drop(first_alias);
    assert!(weak.upgrade().is_some());
    drop(second_alias);
    assert!(weak.upgrade().is_none());
    parent.remove();
}

/// Prepare content while every borrowed callback operand is still available.
fn prepare_owned(
    aliases: Rc<RefCell<Vec<Rc<Row>>>>,
) -> (ToolRenderResult, std::rc::Weak<Cell<i32>>) {
    super::reset();
    maestro_web::register_tool_renderer(
        "owned",
        maestro_web::tools::types::ToolRendererBinding::from_raw(Rc::new(Renderer(aliases))),
    );
    let original = Rc::new(Cell::new(42));
    let weak = Rc::downgrade(&original);
    let message = super::message(Some(original));
    let params = String::from("owned caption");
    let prepared = maestro_web::render_tool(
        "owned",
        Some(maestro_web::tools::types::ToolParams::typed(
            &params,
            |_| unreachable!(),
        )),
        Some(&message),
        false,
    )
    .unwrap();
    (prepared, weak)
}

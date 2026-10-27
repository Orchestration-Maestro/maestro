//! Source-owned panel connection callbacks with weak typed-root ownership.
use super::{Options, State};
use crate::{
    dom::{self, DomView, Link, Property, Reflected},
    tools::types::ToolUiError,
};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};
use wasm_bindgen::JsValue;

/// One registered plain panel element.
struct Element {
    /// Weak native element.
    root: Link,
    /// Panel retained by a typed root or a bare source element.
    state: Weak<State>,
    /// Bare-element ownership, with detached children on disconnection.
    owned: Option<Rc<State>>,
    /// Reflected source properties.
    _properties: Reflected,
}
impl custom_element::CustomElement for Element {
    fn connected_callback(&mut self) {
        if let (Some(state), Some(root)) = (self.state.upgrade(), dom::live(&self.root))
            && (self.owned.is_some() || state.owns(&self.root))
        {
            *state.root.borrow_mut() = Some(Link::new(&root));
            let _ = root.class_list().add_1("maestro-artifacts-panel");
            state.refresh();
        }
    }
    fn disconnected_callback(&mut self) {
        if let Some(state) = self.state.upgrade()
            && state.owns(&self.root)
        {
            state.disconnect();
        }
    }
    fn attribute_changed_callback(&mut self, name: &str, _: JsValue, value: JsValue) {
        if name == "collapsed"
            && let Some(state) = self.state.upgrade()
            && (self.owned.is_some() || state.owns(&self.root))
        {
            state.collapsed.set(!value.is_null());
            state.refresh();
        }
    }
}
/// Register the plain panel; the containing screen owns responsive modal composition.
pub(crate) fn define(view: &Rc<DomView>) -> Result<(), ToolUiError> {
    let view = Rc::clone(view);
    dom::define_element(
        "artifacts-panel",
        vec!["collapsed".into()],
        &[
            Property::Flag("collapsed", "collapsed"),
            Property::Flag("overlay", "overlay"),
        ],
        move |root, properties| {
            let provided = dom::take_creating::<Weak<State>>().and_then(|state| state.upgrade());
            let owned = provided.is_none();
            let state = provided.unwrap_or_else(|| {
                State::new(
                    &view,
                    Rc::new(Cell::new(false)),
                    Rc::new(RefCell::new(None)),
                    Rc::new(Options::default()),
                )
            });
            Element {
                root: Link::new(&root),
                state: Rc::downgrade(&state),
                owned: owned.then_some(state),
                _properties: properties,
            }
        },
    )
    .map_err(|error| ToolUiError::native(&error))
}

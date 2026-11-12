//! Independently typed state shared by a row's renderer slots.
use serde_json::json;
use std::{
    any::{Any, TypeId},
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
};

/// A shared row owner; clones alias the same typed state handles.
#[derive(Clone, Default)]
pub struct ToolRendererState {
    /// One handle per concrete state type.
    states: Rc<RefCell<HashMap<TypeId, Rc<dyn Any>>>>,
}
impl ToolRendererState {
    /// Return the same handle for the same type, initializing JSON to `{}`.
    /// Caller defaults run without borrowing the state map.
    #[must_use]
    pub fn state<S: Default + 'static>(&self) -> Rc<RefCell<S>> {
        let existing = self.states.borrow().get(&TypeId::of::<S>()).cloned();
        if let Some(state) = existing.and_then(|state| state.downcast::<RefCell<S>>().ok()) {
            return state;
        }
        let json: Rc<dyn Any> = Rc::new(RefCell::new(json!({})));
        let state = json
            .downcast::<RefCell<S>>()
            .unwrap_or_else(|_| Rc::new(RefCell::new(S::default())));
        let published = self
            .states
            .borrow_mut()
            .entry(TypeId::of::<S>())
            .or_insert_with(|| state.clone())
            .clone();
        published.downcast::<RefCell<S>>().unwrap_or(state)
    }
}

//! Caller-supplied editor contracts.
use crate::{Component, editor::completion::autocomplete::AutocompleteProvider};
use std::{cell::RefCell, rc::Rc};
/// Caller-supplied logical editor content and optional capabilities.
#[allow(clippy::type_complexity)]
pub trait EditorComponent: Component {
    /// Get text.
    fn get_text(&self) -> String;
    /// Set text.
    fn set_text(&mut self, text: String);
    /// Assignable shared submit callback, absent until supplied.
    fn on_submit(&mut self) -> &mut Option<Rc<dyn Fn(String)>>;
    /// Assignable shared change callback, absent until supplied.
    fn on_change(&mut self) -> &mut Option<Rc<dyn Fn(String)>>;
    /// Optional history insertion; absence remains absence.
    fn add_to_history(&mut self) -> Option<Box<dyn FnMut(&str) + '_>> {
        None
    }
    /// Optional cursor insertion.
    fn insert_text_at_cursor(&mut self) -> Option<Box<dyn FnMut(&str) + '_>> {
        None
    }
    /// Optional expanded content; callers own the get-text fallback.
    fn get_expanded_text(&self) -> Option<Box<dyn Fn() -> String + '_>> {
        None
    }
    /// Optional provider assignment.
    fn set_autocomplete_provider(
        &mut self,
    ) -> Option<Box<dyn FnMut(Rc<RefCell<dyn AutocompleteProvider>>) + '_>> {
        None
    }
    /// Assignable shared border style, including assignment while absent.
    fn border_color(&mut self) -> &mut Option<Rc<dyn Fn(&str) -> String>>;
    /// Optional horizontal padding assignment.
    fn set_padding_x(&mut self) -> Option<Box<dyn FnMut(usize) + '_>> {
        None
    }
    /// Optional completion-list height assignment.
    fn set_autocomplete_max_visible(&mut self) -> Option<Box<dyn FnMut(usize) + '_>> {
        None
    }
}

//! The contract of a replaceable text editor component.

use std::rc::Rc;

use crate::autocomplete::AutocompleteProvider;
use crate::tui::{Component, InputHandler};

/// Receives the editor text when the user submits or changes it.
pub type TextCallback = Box<dyn FnMut(&str)>;

/// Paints text with the border colour.
pub type BorderColor = Rc<dyn Fn(&str) -> String>;

/// Callbacks an editor invokes; each starts absent and can be replaced at any time.
#[derive(Default)]
pub struct EditorCallbacks {
    /// Called with the text when the user submits.
    pub on_submit: Option<TextCallback>,
    /// Called with the text after every change.
    pub on_change: Option<TextCallback>,
    /// Paints the editor border.
    pub border_color: Option<BorderColor>,
}

/// A text editor that extensions can substitute for the built-in one.
///
/// Optional operations return `None` when the editor does not support them. Those that
/// act return `Some(())` once carried out; [`get_expanded_text`] returns the expanded text.
///
/// [`get_expanded_text`]: EditorComponent::get_expanded_text
pub trait EditorComponent: Component + InputHandler {
    /// The cancellation signal type of the completion providers the editor accepts.
    type Signal: ?Sized;

    /// The current text.
    fn get_text(&self) -> String;

    /// Replaces the text.
    fn set_text(&mut self, text: &str);

    /// The callbacks the editor invokes.
    fn callbacks(&mut self) -> &mut EditorCallbacks;

    /// Adds text to the history used for up and down navigation.
    fn add_to_history(&mut self, _text: &str) -> Option<()> {
        None
    }

    /// Inserts text at the cursor.
    fn insert_text_at_cursor(&mut self, _text: &str) -> Option<()> {
        None
    }

    /// The text with markers such as pastes expanded, or `None` when the editor does not
    /// support expansion and [`get_text`] is all there is.
    ///
    /// [`get_text`]: EditorComponent::get_text
    fn get_expanded_text(&self) -> Option<String> {
        None
    }

    /// Sets the completion provider.
    fn set_autocomplete_provider(
        &mut self,
        _provider: Rc<dyn AutocompleteProvider<Signal = Self::Signal>>,
    ) -> Option<()> {
        None
    }

    /// Sets the horizontal padding.
    fn set_padding_x(&mut self, _padding: usize) -> Option<()> {
        None
    }

    /// Sets how many completion items are visible at once.
    fn set_autocomplete_max_visible(&mut self, _max_visible: usize) -> Option<()> {
        None
    }
}

//! The contract of a replaceable text editor component.

use std::rc::Rc;

use maestro_cancellation::Cancellation;

use crate::Editor;
use crate::autocomplete::AutocompleteProvider;
use crate::tui::{Component, InputHandler};

/// Receives the editor text when the user submits or changes it.
pub type TextCallback = Rc<dyn Fn(&str)>;

/// Paints text with the border colour.
pub type BorderColor = Rc<dyn Fn(&str) -> String>;

/// A text editor that extensions can substitute for the built-in one.
///
/// Optional operations return `None` when the editor does not support them. Those that
/// act return `Some(())` once carried out; [`get_expanded_text`] returns the expanded text.
/// Every operation takes `&self`, so a handle shared with its callbacks stays usable;
/// editors keep their state behind interior mutability.
///
/// [`get_expanded_text`]: EditorComponent::get_expanded_text
pub trait EditorComponent: Component + InputHandler {
    /// The cancellation signal type of the completion providers the editor accepts.
    type Signal: ?Sized;

    /// The current text.
    fn get_text(&self) -> String;

    /// Replaces the text.
    fn set_text(&self, text: &str);

    /// The callback invoked with the text when the user submits, if any.
    fn on_submit(&self) -> Option<TextCallback>;

    /// Replaces or removes the submission callback.
    fn set_on_submit(&self, callback: Option<TextCallback>);

    /// The callback invoked with the text after every change, if any.
    fn on_change(&self) -> Option<TextCallback>;

    /// Replaces or removes the change callback.
    fn set_on_change(&self, callback: Option<TextCallback>);

    /// Adds text to the history used for up and down navigation.
    fn add_to_history(&self, _text: &str) -> Option<()> {
        None
    }

    /// Inserts text at the cursor.
    fn insert_text_at_cursor(&self, _text: &str) -> Option<()> {
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
        &self,
        _provider: Rc<dyn AutocompleteProvider<Signal = Self::Signal>>,
    ) -> Option<()> {
        None
    }

    /// The border painter, or `None` when the editor has none to share.
    fn border_color(&self) -> Option<BorderColor> {
        None
    }

    /// Sets the border painter.
    fn set_border_color(&self, _color: BorderColor) -> Option<()> {
        None
    }

    /// Sets the horizontal padding.
    fn set_padding_x(&self, _padding: f64) -> Option<()> {
        None
    }

    /// Sets how many completion items are visible at once.
    fn set_autocomplete_max_visible(&self, _max_visible: f64) -> Option<()> {
        None
    }
}

impl EditorComponent for Editor {
    type Signal = Cancellation;

    fn get_text(&self) -> String {
        Editor::get_text(self)
    }

    fn set_text(&self, text: &str) {
        Editor::set_text(self, text);
    }

    fn on_submit(&self) -> Option<TextCallback> {
        Editor::on_submit(self)
    }

    fn set_on_submit(&self, callback: Option<TextCallback>) {
        Editor::set_on_submit(self, callback);
    }

    fn on_change(&self) -> Option<TextCallback> {
        Editor::on_change(self)
    }

    fn set_on_change(&self, callback: Option<TextCallback>) {
        Editor::set_on_change(self, callback);
    }

    fn add_to_history(&self, text: &str) -> Option<()> {
        Editor::add_to_history(self, text);
        Some(())
    }

    fn insert_text_at_cursor(&self, text: &str) -> Option<()> {
        Editor::insert_text_at_cursor(self, text);
        Some(())
    }

    fn get_expanded_text(&self) -> Option<String> {
        Some(Editor::get_expanded_text(self))
    }

    fn set_autocomplete_provider(
        &self,
        provider: Rc<dyn AutocompleteProvider<Signal = Cancellation>>,
    ) -> Option<()> {
        Editor::set_autocomplete_provider(self, provider);
        Some(())
    }

    fn border_color(&self) -> Option<BorderColor> {
        Some(Editor::border_color(self))
    }

    fn set_border_color(&self, color: BorderColor) -> Option<()> {
        Editor::set_border_color(self, color);
        Some(())
    }

    fn set_padding_x(&self, padding: f64) -> Option<()> {
        Editor::set_padding_x(self, padding);
        Some(())
    }

    fn set_autocomplete_max_visible(&self, max_visible: f64) -> Option<()> {
        Editor::set_autocomplete_max_visible(self, max_visible);
        Some(())
    }
}

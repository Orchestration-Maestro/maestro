//! Controlled editors: one with every optional hook and one with only the required ones.

use std::cell::Cell;
use std::rc::Rc;

use maestro_tui::autocomplete::AutocompleteProvider;
use maestro_tui::editor_component::{EditorCallbacks, EditorComponent};
use maestro_tui::tui::{Component, InputHandler};

/// The provider type the rich editor accepts.
pub type FlagProvider = dyn AutocompleteProvider<Signal = Cell<bool>>;

/// An editor that supports every optional hook.
pub struct Rich {
    /// Current text.
    pub text: String,
    /// Callbacks under test.
    pub callbacks: EditorCallbacks,
    /// Text added to history.
    pub history: Vec<String>,
    /// Horizontal padding.
    pub padding: usize,
    /// Visible completion items.
    pub visible: usize,
    /// Installed provider.
    pub provider: Option<Rc<FlagProvider>>,
}

impl Rich {
    /// Creates an empty editor.
    pub fn new() -> Self {
        Self {
            text: String::new(),
            callbacks: EditorCallbacks::default(),
            history: Vec::new(),
            padding: 0,
            visible: 0,
            provider: None,
        }
    }
}

impl Component for Rich {
    fn render(&mut self, _width: usize) -> Vec<String> {
        let border = self
            .callbacks
            .border_color
            .as_ref()
            .map_or_else(|| "-".to_owned(), |paint| paint("-"));
        vec![border, self.text.clone()]
    }

    fn input_handler(&mut self) -> Option<&mut dyn InputHandler> {
        Some(self)
    }
}

impl InputHandler for Rich {
    fn handle_input(&mut self, data: &str) {
        if data == "\r" {
            if let Some(submit) = self.callbacks.on_submit.as_mut() {
                submit(&self.text);
            }
            return;
        }
        self.text.push_str(data);
        if let Some(change) = self.callbacks.on_change.as_mut() {
            change(&self.text);
        }
    }
}

impl EditorComponent for Rich {
    type Signal = Cell<bool>;

    fn get_text(&self) -> String {
        self.text.clone()
    }

    fn set_text(&mut self, text: &str) {
        text.clone_into(&mut self.text);
    }

    fn callbacks(&mut self) -> &mut EditorCallbacks {
        &mut self.callbacks
    }

    fn add_to_history(&mut self, text: &str) -> Option<()> {
        self.history.push(text.to_owned());
        Some(())
    }

    fn insert_text_at_cursor(&mut self, text: &str) -> Option<()> {
        self.text.push_str(text);
        Some(())
    }

    fn get_expanded_text(&self) -> Option<String> {
        Some(self.text.replace("[paste]", "pasted text"))
    }

    fn set_autocomplete_provider(&mut self, provider: Rc<FlagProvider>) -> Option<()> {
        self.provider = Some(provider);
        Some(())
    }

    fn set_padding_x(&mut self, padding: usize) -> Option<()> {
        self.padding = padding;
        Some(())
    }

    fn set_autocomplete_max_visible(&mut self, max_visible: usize) -> Option<()> {
        self.visible = max_visible;
        Some(())
    }
}

/// An editor that implements only the required members.
pub struct Bare {
    /// Current text.
    pub text: String,
    /// Callbacks storage.
    pub callbacks: EditorCallbacks,
}

impl Component for Bare {
    fn render(&mut self, _width: usize) -> Vec<String> {
        vec![self.text.clone()]
    }

    fn input_handler(&mut self) -> Option<&mut dyn InputHandler> {
        Some(self)
    }
}

impl InputHandler for Bare {
    fn handle_input(&mut self, data: &str) {
        self.text.push_str(data);
    }
}

impl EditorComponent for Bare {
    type Signal = std::cell::RefCell<String>;

    fn get_text(&self) -> String {
        self.text.clone()
    }

    fn set_text(&mut self, text: &str) {
        text.clone_into(&mut self.text);
    }

    fn callbacks(&mut self) -> &mut EditorCallbacks {
        &mut self.callbacks
    }
}

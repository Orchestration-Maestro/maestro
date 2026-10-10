//! Controlled editors: one with every optional hook and one with only the required ones.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use maestro_tui::editor_component::{BorderColor, TextCallback};
use maestro_tui::tui::InputHandler;
use maestro_tui::{AutocompleteProvider, Component, EditorComponent};

/// The provider type the rich editor accepts.
pub type FlagProvider = dyn AutocompleteProvider<Signal = Cell<bool>>;

/// An editor that supports every optional hook.
pub struct Rich {
    /// Current text.
    pub text: RefCell<String>,
    /// Submission callback slot.
    pub submit: RefCell<Option<TextCallback>>,
    /// Change callback slot.
    pub change: RefCell<Option<TextCallback>>,
    /// Border painter slot.
    pub border: RefCell<Option<BorderColor>>,
    /// Text added to history.
    pub history: RefCell<Vec<String>>,
    /// Horizontal padding.
    pub padding: Cell<f64>,
    /// Visible completion items.
    pub visible: Cell<f64>,
    /// Installed provider.
    pub provider: RefCell<Option<Rc<FlagProvider>>>,
}

impl Rich {
    /// Creates an empty editor.
    pub fn new() -> Self {
        Self {
            text: RefCell::default(),
            submit: RefCell::default(),
            change: RefCell::default(),
            border: RefCell::default(),
            history: RefCell::default(),
            padding: Cell::new(0.0),
            visible: Cell::new(0.0),
            provider: RefCell::default(),
        }
    }

    /// Runs the callback in `slot` with the current text, holding no cell guard meanwhile.
    fn notify(&self, slot: &RefCell<Option<TextCallback>>) {
        let callback = slot.borrow().clone();
        if let Some(callback) = callback {
            let text = self.text.borrow().clone();
            callback(&text);
        }
    }
}

impl Component for Rich {
    fn render(&self, _width: usize) -> Vec<String> {
        let paint = self.border.borrow().clone();
        let border = paint.map_or_else(|| "-".to_owned(), |paint| paint("-"));
        vec![border, self.text.borrow().clone()]
    }

    fn input_handler(&self) -> Option<&dyn InputHandler> {
        Some(self)
    }
}

impl InputHandler for Rich {
    fn handle_input(&self, data: &str) {
        if data == "\r" {
            self.notify(&self.submit);
            return;
        }
        self.text.borrow_mut().push_str(data);
        self.notify(&self.change);
    }
}

impl EditorComponent for Rich {
    type Signal = Cell<bool>;

    fn get_text(&self) -> String {
        self.text.borrow().clone()
    }

    fn set_text(&self, text: &str) {
        text.clone_into(&mut self.text.borrow_mut());
    }

    fn on_submit(&self) -> Option<TextCallback> {
        self.submit.borrow().clone()
    }

    fn set_on_submit(&self, callback: Option<TextCallback>) {
        self.submit.replace(callback);
    }

    fn on_change(&self) -> Option<TextCallback> {
        self.change.borrow().clone()
    }

    fn set_on_change(&self, callback: Option<TextCallback>) {
        self.change.replace(callback);
    }

    fn add_to_history(&self, text: &str) -> Option<()> {
        self.history.borrow_mut().push(text.to_owned());
        Some(())
    }

    fn insert_text_at_cursor(&self, text: &str) -> Option<()> {
        self.text.borrow_mut().push_str(text);
        Some(())
    }

    fn get_expanded_text(&self) -> Option<String> {
        Some(self.text.borrow().replace("[paste]", "pasted text"))
    }

    fn set_autocomplete_provider(&self, provider: Rc<FlagProvider>) -> Option<()> {
        self.provider.replace(Some(provider));
        Some(())
    }

    fn border_color(&self) -> Option<BorderColor> {
        self.border.borrow().clone()
    }

    fn set_border_color(&self, color: BorderColor) -> Option<()> {
        self.border.replace(Some(color));
        Some(())
    }

    fn set_padding_x(&self, padding: f64) -> Option<()> {
        self.padding.set(padding);
        Some(())
    }

    fn set_autocomplete_max_visible(&self, max_visible: f64) -> Option<()> {
        self.visible.set(max_visible);
        Some(())
    }
}

/// An editor that implements only the required members.
pub struct Bare {
    /// Current text.
    pub text: RefCell<String>,
    /// Submission callback slot.
    pub submit: RefCell<Option<TextCallback>>,
    /// Change callback slot.
    pub change: RefCell<Option<TextCallback>>,
}

impl Component for Bare {
    fn render(&self, _width: usize) -> Vec<String> {
        vec![self.text.borrow().clone()]
    }

    fn input_handler(&self) -> Option<&dyn InputHandler> {
        Some(self)
    }
}

impl InputHandler for Bare {
    fn handle_input(&self, data: &str) {
        self.text.borrow_mut().push_str(data);
    }
}

impl EditorComponent for Bare {
    type Signal = RefCell<String>;

    fn get_text(&self) -> String {
        self.text.borrow().clone()
    }

    fn set_text(&self, text: &str) {
        text.clone_into(&mut self.text.borrow_mut());
    }

    fn on_submit(&self) -> Option<TextCallback> {
        self.submit.borrow().clone()
    }

    fn set_on_submit(&self, callback: Option<TextCallback>) {
        self.submit.replace(callback);
    }

    fn on_change(&self) -> Option<TextCallback> {
        self.change.borrow().clone()
    }

    fn set_on_change(&self, callback: Option<TextCallback>) {
        self.change.replace(callback);
    }
}

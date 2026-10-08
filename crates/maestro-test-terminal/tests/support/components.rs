//! Components that record how the frame writer treats them.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use maestro_tui::tui::InputHandler;
use maestro_tui::{CURSOR_MARKER, Component, FocusFlag, Focusable};

/// Called at the start of each render with the render count.
type RenderHook = Rc<dyn Fn(usize)>;

/// Called with each chunk of input a component receives.
type InputHook = Rc<dyn Fn(&str)>;

/// Called each time a component is invalidated.
type InvalidateHook = Rc<dyn Fn()>;

/// Which optional capabilities a [`Probe`] offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capabilities {
    /// It can hold focus and accepts input.
    Full,
    /// It can hold focus but accepts no input.
    FocusOnly,
    /// It has neither capability.
    Passive,
}

/// A component with fixed lines that records renders, invalidations, focus and input.
pub struct Probe {
    /// Lines returned by every render.
    pub lines: RefCell<Vec<String>>,
    /// Chunks of input received, in order.
    pub inputs: RefCell<Vec<String>>,
    /// Whether the component currently has focus.
    pub focus: FocusFlag,
    /// Which optional capabilities the component offers.
    pub capabilities: Cell<Capabilities>,
    /// Whether the component asks for key-release events.
    pub wants_release: Cell<bool>,
    /// Whether a focused component marks the end of its last line as the cursor position.
    pub emits_marker: Cell<bool>,
    /// How many times it was invalidated.
    pub invalidated: Cell<usize>,
    /// How many times it rendered.
    pub renders: Cell<usize>,
    /// Called at the start of each render.
    on_render: RefCell<Option<RenderHook>>,
    /// Called with each chunk of input the component receives.
    on_input: RefCell<Option<InputHook>>,
    /// Called each time the component is invalidated.
    on_invalidate: RefCell<Option<InvalidateHook>>,
}

impl Probe {
    /// A shared component that renders `lines` and has focus and input capabilities.
    pub fn shared(lines: &[&str]) -> Rc<Self> {
        Rc::new(Self {
            lines: RefCell::new(lines.iter().map(|line| (*line).to_owned()).collect()),
            inputs: RefCell::default(),
            focus: FocusFlag::default(),
            capabilities: Cell::new(Capabilities::Full),
            wants_release: Cell::new(false),
            emits_marker: Cell::new(false),
            invalidated: Cell::new(0),
            renders: Cell::new(0),
            on_render: RefCell::default(),
            on_input: RefCell::default(),
            on_invalidate: RefCell::default(),
        })
    }

    /// Replaces the rendered lines.
    pub fn set_lines(&self, lines: &[String]) {
        lines.clone_into(&mut self.lines.borrow_mut());
    }

    /// Calls `callback` at the start of each render with the render count.
    pub fn on_render(&self, callback: impl Fn(usize) + 'static) {
        *self.on_render.borrow_mut() = Some(Rc::new(callback));
    }

    /// Calls `callback` with each chunk of input the component receives.
    pub fn on_input(&self, callback: impl Fn(&str) + 'static) {
        *self.on_input.borrow_mut() = Some(Rc::new(callback));
    }

    /// Calls `callback` each time the component is invalidated.
    pub fn on_invalidate(&self, callback: impl Fn() + 'static) {
        *self.on_invalidate.borrow_mut() = Some(Rc::new(callback));
    }

    /// Drops every callback, which may hold the writer that holds this component.
    pub fn clear_callbacks(&self) {
        self.on_render.take();
        self.on_input.take();
        self.on_invalidate.take();
    }
}

impl Component for Probe {
    fn render(&self, _width: usize) -> Vec<String> {
        self.renders.set(self.renders.get() + 1);
        let on_render = self.on_render.borrow().clone();
        if let Some(on_render) = on_render {
            on_render(self.renders.get());
        }
        let mut lines = self.lines.borrow().clone();
        if let Some(last) = lines
            .last_mut()
            .filter(|_| self.focus.get() && self.emits_marker.get())
        {
            last.push_str(CURSOR_MARKER);
        }
        lines
    }

    fn invalidate(&self) {
        self.invalidated.set(self.invalidated.get() + 1);
        let on_invalidate = self.on_invalidate.borrow().clone();
        if let Some(on_invalidate) = on_invalidate {
            on_invalidate();
        }
    }

    fn input_handler(&self) -> Option<&dyn InputHandler> {
        (self.capabilities.get() == Capabilities::Full).then_some(self as &dyn InputHandler)
    }

    fn wants_key_release(&self) -> bool {
        self.wants_release.get()
    }

    fn focusable(&self) -> Option<&dyn Focusable> {
        (self.capabilities.get() != Capabilities::Passive).then_some(self as &dyn Focusable)
    }
}

impl InputHandler for Probe {
    fn handle_input(&self, data: &str) {
        self.inputs.borrow_mut().push(data.to_owned());
        let on_input = self.on_input.borrow().clone();
        if let Some(on_input) = on_input {
            on_input(data);
        }
    }
}

impl Focusable for Probe {
    fn focus_flag(&self) -> &FocusFlag {
        &self.focus
    }
}

/// A component that keeps the lines it rendered until it is invalidated, as a component
/// with an expensive rendering does.
pub struct Cached {
    /// The text the next uncached render shows.
    text: RefCell<String>,
    /// The lines of the last render, until the next invalidation.
    lines: RefCell<Option<Vec<String>>>,
}

impl Cached {
    /// A shared component whose first render shows `text`.
    pub fn shared(text: &str) -> Rc<Self> {
        Rc::new(Self {
            text: RefCell::new(text.to_owned()),
            lines: RefCell::default(),
        })
    }

    /// Changes the text the next uncached render shows.
    pub fn set_text(&self, text: &str) {
        text.clone_into(&mut self.text.borrow_mut());
    }
}

impl Component for Cached {
    fn render(&self, _width: usize) -> Vec<String> {
        if let Some(lines) = &*self.lines.borrow() {
            return lines.clone();
        }
        let lines = vec![self.text.borrow().clone()];
        *self.lines.borrow_mut() = Some(lines.clone());
        lines
    }

    fn invalidate(&self) {
        self.lines.take();
    }
}

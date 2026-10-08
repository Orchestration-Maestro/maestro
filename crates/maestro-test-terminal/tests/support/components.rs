//! Components that record how the frame writer treats them.

use std::cell::RefCell;
use std::rc::Rc;

use maestro_tui::tui::InputHandler;
use maestro_tui::{CURSOR_MARKER, Component, FocusFlag, Focusable};

/// Receives each chunk of input a [`Probe`] gets.
pub type InputObserver = Box<dyn FnMut(&str)>;

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
    pub lines: Vec<String>,
    /// Chunks of input received, in order.
    pub inputs: Vec<String>,
    /// Whether the component currently has focus.
    pub focus: FocusFlag,
    /// Which optional capabilities the component offers.
    pub capabilities: Capabilities,
    /// Whether the component asks for key-release events.
    pub wants_release: bool,
    /// Whether a focused component marks the end of its last line as the cursor position.
    pub emits_marker: bool,
    /// How many times it was invalidated.
    pub invalidated: usize,
    /// How many times it rendered.
    pub renders: usize,
    /// Called at the start of each render with the render count.
    pub on_render: Option<Box<dyn FnMut(usize)>>,
    /// Called with each chunk of input the component receives.
    pub on_input: Option<InputObserver>,
    /// Called each time the component is invalidated.
    pub on_invalidate: Option<Box<dyn FnMut()>>,
}

impl Probe {
    /// A shared component that renders `lines` and has focus and input capabilities.
    pub fn shared(lines: &[&str]) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            lines: lines.iter().map(|line| (*line).to_owned()).collect(),
            inputs: Vec::new(),
            focus: FocusFlag::default(),
            capabilities: Capabilities::Full,
            wants_release: false,
            emits_marker: false,
            invalidated: 0,
            renders: 0,
            on_render: None,
            on_input: None,
            on_invalidate: None,
        }))
    }

    /// Replaces the rendered lines.
    pub fn set_lines(&mut self, lines: &[String]) {
        self.lines = lines.to_vec();
    }
}

impl Component for Probe {
    fn render(&mut self, _width: usize) -> Vec<String> {
        self.renders += 1;
        if let Some(on_render) = self.on_render.as_mut() {
            on_render(self.renders);
        }
        let mut lines = self.lines.clone();
        if let Some(last) = lines
            .last_mut()
            .filter(|_| self.focus.get() && self.emits_marker)
        {
            last.push_str(CURSOR_MARKER);
        }
        lines
    }

    fn invalidate(&mut self) {
        self.invalidated += 1;
        if let Some(on_invalidate) = self.on_invalidate.as_mut() {
            on_invalidate();
        }
    }

    fn input_handler(&mut self) -> Option<&mut dyn InputHandler> {
        (self.capabilities == Capabilities::Full).then_some(self as &mut dyn InputHandler)
    }

    fn wants_key_release(&self) -> bool {
        self.wants_release
    }

    fn focusable(&self) -> Option<&dyn Focusable> {
        (self.capabilities != Capabilities::Passive).then_some(self as &dyn Focusable)
    }
}

impl InputHandler for Probe {
    fn handle_input(&mut self, data: &str) {
        self.inputs.push(data.to_owned());
        if let Some(on_input) = self.on_input.as_mut() {
            on_input(data);
        }
    }
}

impl Focusable for Probe {
    fn focus_flag(&self) -> &FocusFlag {
        &self.focus
    }
}

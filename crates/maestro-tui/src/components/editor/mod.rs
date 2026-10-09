#![doc = include_str!("../../../../../docs/terminal/editor.md")]
mod input;
mod render;
mod text;
mod wrapping;
use crate::{
    Component, FocusFlag, Focusable, SelectListTheme, TUI,
    autocomplete::CursorPosition,
    editor_component::BorderColor,
    tui::{InputHandler, TerminalHandle},
    undo_stack::UndoStack,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
pub use wrapping::{TextChunk, word_wrap_line};
/// Styles the border and retains selection styling for completion.
pub struct EditorTheme {
    /// Styles horizontal borders and scroll labels.
    pub border_color: BorderColor,
    /// Selection styling retained with the editor.
    pub select_list: SelectListTheme,
}
/// Optional horizontal padding and completion-list size.
#[derive(Clone, Copy, Default)]
pub struct EditorOptions {
    /// Finite side padding is floored and clamped at zero; nonfinite values use zero.
    pub padding_x: Option<f64>,
    /// Finite completion maxima are floored and clamped to 3..20; nonfinite values use 5.
    pub autocomplete_max_visible: Option<f64>,
}
/// Synchronous text notification.
type TextCallback = Rc<dyn Fn(&str)>;
/// Multiline editable terminal component.
pub struct Editor {
    /// Buffer, cursor and snapshots.
    state: RefCell<Editing>,
    /// Focus controlled by the writer.
    focus: FocusFlag,
    /// Live terminal dimensions.
    terminal: TerminalHandle,
    /// Weak request to the writer.
    request: Rc<dyn Fn()>,
    /// Supplied theme retains all callable identities.
    _theme: EditorTheme,
    /// Replaceable border styling.
    border: RefCell<BorderColor>,
    /// Change notification.
    change: RefCell<Option<TextCallback>>,
    /// Submission notification.
    submit: RefCell<Option<TextCallback>>,
    /// Submission gate.
    disabled: Cell<bool>,
    /// Requested side padding.
    padding: Cell<f64>,
    /// Completion-list maximum.
    maximum: Cell<usize>,
    /// First visible layout line.
    scroll: Cell<usize>,
}
/// Text and byte cursor captured together.
#[derive(Clone)]
struct Buffer {
    /// Logical lines, including empty lines.
    lines: Vec<String>,
    /// Current byte position.
    cursor: CursorPosition,
}
impl Default for Buffer {
    fn default() -> Self {
        Self {
            lines: vec![String::new()],
            cursor: CursorPosition { line: 0, col: 0 },
        }
    }
}
/// Mutable editing transitions.
#[derive(Default)]
struct Editing {
    /// Current buffer.
    current: Buffer,
    /// Shared snapshot owner.
    undo: UndoStack<Buffer>,
    /// Whether consecutive word typing coalesces.
    typing: bool,
}
impl Editor {
    /// Creates an empty editor retaining live terminal dimensions without retaining its writer.
    #[must_use]
    pub fn new(tui: &TUI, theme: EditorTheme, options: EditorOptions) -> Self {
        Self {
            state: RefCell::default(),
            focus: FocusFlag::default(),
            terminal: Rc::clone(tui.terminal()),
            request: tui.weak_render_request(),
            border: RefCell::new(Rc::clone(&theme.border_color)),
            _theme: theme,
            change: RefCell::default(),
            submit: RefCell::default(),
            disabled: Cell::new(false),
            padding: Cell::new(normalize_padding(options.padding_x.unwrap_or(0.0))),
            maximum: Cell::new(normalize(
                options.autocomplete_max_visible.unwrap_or(5.0),
                3,
                20,
                5,
            )),
            scroll: Cell::new(0),
        }
    }
    /// Returns the requested horizontal padding.
    #[must_use]
    pub fn get_padding_x(&self) -> f64 {
        self.padding.get()
    }
    /// Changes padding and requests a frame only when the normalized value changes.
    pub fn set_padding_x(&self, padding: f64) {
        let value = normalize_padding(padding);
        if self.padding.replace(value).partial_cmp(&value) != Some(std::cmp::Ordering::Equal) {
            (self.request)();
        }
    }
    /// Returns the retained completion-list maximum.
    #[must_use]
    pub fn get_autocomplete_max_visible(&self) -> usize {
        self.maximum.get()
    }
    /// Changes the completion-list maximum, requesting a frame on change.
    pub fn set_autocomplete_max_visible(&self, maximum: f64) {
        let value = normalize(maximum, 3, 20, 5);
        if self.maximum.replace(value) != value {
            (self.request)();
        }
    }
    /// Returns joined logical lines.
    #[must_use]
    pub fn get_text(&self) -> String {
        self.state.borrow().current.lines.join("\n")
    }
    /// Returns independent owned logical lines.
    #[must_use]
    pub fn get_lines(&self) -> Vec<String> {
        self.state.borrow().current.lines.clone()
    }
    /// Returns the stored byte cursor.
    #[must_use]
    pub fn get_cursor(&self) -> CursorPosition {
        self.state.borrow().current.cursor
    }
    /// Returns a retained border callable.
    #[must_use]
    pub fn border_color(&self) -> BorderColor {
        self.border.borrow().clone()
    }
    /// Replaces border styling without requesting a frame.
    pub fn set_border_color(&self, color: BorderColor) {
        let old = self.border.replace(color);
        drop(old);
    }
    /// Returns a retained change callable.
    #[must_use]
    pub fn on_change(&self) -> Option<TextCallback> {
        self.change.borrow().clone()
    }
    /// Replaces or removes change notification.
    pub fn set_on_change(&self, callback: Option<TextCallback>) {
        let old = self.change.replace(callback);
        drop(old);
    }
    /// Returns a retained submission callable.
    #[must_use]
    pub fn on_submit(&self) -> Option<TextCallback> {
        self.submit.borrow().clone()
    }
    /// Replaces or removes submission notification.
    pub fn set_on_submit(&self, callback: Option<TextCallback>) {
        let old = self.submit.replace(callback);
        drop(old);
    }
    /// Returns whether submission is disabled.
    #[must_use]
    pub fn disable_submit(&self) -> bool {
        self.disabled.get()
    }
    /// Gates submission without changing text.
    pub fn set_disable_submit(&self, disabled: bool) {
        self.disabled.set(disabled);
    }
    /// Notifies from a committed buffer without holding a state borrow.
    fn notify(&self) {
        if let Some(callback) = self.on_change() {
            let text = self.get_text();
            callback(&text);
        }
    }
}
/// Retains finite authored padding without a native cell-count bound.
fn normalize_padding(value: f64) -> f64 {
    if value.is_finite() {
        value.floor().max(0.0)
    } else {
        0.0
    }
}
/// Floors and saturates finite native counts; nonfinite values use the fallback.
fn normalize(value: f64, minimum: usize, maximum: usize, fallback: usize) -> usize {
    if value.is_finite() {
        value
            .floor()
            .max(0.0)
            .to_string()
            .parse::<usize>()
            .unwrap_or(usize::MAX)
            .clamp(minimum, maximum)
    } else {
        fallback
    }
}
impl Focusable for Editor {
    fn focus_flag(&self) -> &FocusFlag {
        &self.focus
    }
}
impl Component for Editor {
    fn render(&self, width: usize) -> Vec<String> {
        self.draw(width)
    }
    fn input_handler(&self) -> Option<&dyn InputHandler> {
        Some(self)
    }
    fn focusable(&self) -> Option<&dyn Focusable> {
        Some(self)
    }
}

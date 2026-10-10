#![doc = include_str!("../../../../../docs/terminal/editor.md")]
mod completion;
mod completion_input;
mod history;
mod input;
mod kill;
mod markers;
mod navigation;
mod paste;
mod render;
mod text;
mod wrapping;
use crate::{
    Component, FocusFlag, Focusable, SelectListTheme, TUI,
    autocomplete::{AutocompleteProvider, CursorPosition},
    editor_component::{BorderColor, TextCallback},
    tui::{InputHandler, TerminalHandle, TuiRuntime},
    undo_stack::UndoStack,
};
use completion::Completion;
use maestro_cancellation::Cancellation;
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
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
/// Multiline editable terminal component.
///
/// A handle on one retained owner; delayed and asynchronous completion work holds
/// only a weak reference to that owner.
pub struct Editor {
    /// The single retained state.
    owner: Rc<Owner>,
}
/// Retained editor state shared by the handle and its weakly held completion work.
struct Owner {
    /// Weak self reference for work that outlives a call.
    me: Weak<Owner>,
    /// Buffer, cursor and snapshots.
    state: RefCell<Editing>,
    /// Focus controlled by the writer.
    focus: FocusFlag,
    /// Live terminal dimensions.
    terminal: TerminalHandle,
    /// Weak request to the writer.
    request: Rc<dyn Fn()>,
    /// Host for the completion debounce and request futures.
    runtime: Rc<dyn TuiRuntime>,
    /// Supplied theme retains all callable identities.
    theme: EditorTheme,
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
    /// Layout width committed by the latest render.
    width: Cell<usize>,
    /// Provider, menu and request state.
    completion: RefCell<Completion>,
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
/// Previous editing action used for coalescing and replacement eligibility.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum LastAction {
    /// No coalescing action.
    #[default]
    None,
    /// Consecutive typed word.
    TypeWord,
    /// Accumulating deleted text.
    Kill,
    /// Eligible inserted ring text.
    Yank,
}
/// Mutable editing transitions.
#[derive(Default)]
struct Editing {
    /// Current buffer.
    current: Buffer,
    /// Shared snapshot owner.
    undo: UndoStack<Buffer>,
    /// Whether consecutive word typing coalesces.
    action: LastAction,
    /// Preferred cell column across shorter visual rows.
    preferred: Option<usize>,
    /// Unrounded absolute cell position before snapping to an atom.
    snapped: Option<usize>,
    /// Prompts retained newest first.
    history: Vec<String>,
    /// Selected history entry, or the current prompt.
    history_index: Option<usize>,
    /// Retained deleted strings.
    ring: crate::kill_ring::KillRing,
    /// Invalidates completion after reentrant editing.
    revision: u64,
    /// Pending direction for a literal jump.
    jump: Option<navigation::Direction>,
    /// Stored large pastes in creation order; paste `n` has marker identifier `n + 1`.
    pastes: Vec<String>,
    /// Input buffered since a bracketed paste started.
    framing: Option<String>,
}
impl Editor {
    /// Creates an empty editor retaining live terminal dimensions without retaining its writer.
    #[must_use]
    pub fn new(tui: &TUI, theme: EditorTheme, options: EditorOptions) -> Self {
        Self {
            owner: Rc::new_cyclic(|me| Owner {
                me: me.clone(),
                state: RefCell::default(),
                focus: FocusFlag::default(),
                terminal: Rc::clone(tui.terminal()),
                request: tui.weak_render_request(),
                runtime: tui.runtime(),
                border: RefCell::new(Rc::clone(&theme.border_color)),
                theme,
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
                width: Cell::new(80),
                completion: RefCell::default(),
            }),
        }
    }
    /// Returns the requested horizontal padding.
    #[must_use]
    pub fn get_padding_x(&self) -> f64 {
        self.owner.padding.get()
    }
    /// Changes padding and requests a frame only when the normalized value changes.
    pub fn set_padding_x(&self, padding: f64) {
        let value = normalize_padding(padding);
        if self.owner.padding.replace(value).partial_cmp(&value) != Some(std::cmp::Ordering::Equal)
        {
            (self.owner.request)();
        }
    }
    /// Returns the retained completion-list maximum.
    #[must_use]
    pub fn get_autocomplete_max_visible(&self) -> usize {
        self.owner.maximum.get()
    }
    /// Changes the completion-list maximum, requesting a frame on change.
    pub fn set_autocomplete_max_visible(&self, maximum: f64) {
        let value = normalize(maximum, 3, 20, 5);
        if self.owner.maximum.replace(value) != value {
            (self.owner.request)();
        }
    }
    /// Returns joined logical lines.
    #[must_use]
    pub fn get_text(&self) -> String {
        self.owner.get_text()
    }
    /// Returns independent owned logical lines.
    #[must_use]
    pub fn get_lines(&self) -> Vec<String> {
        self.owner.state.borrow().current.lines.clone()
    }
    /// Returns the stored byte cursor.
    #[must_use]
    pub fn get_cursor(&self) -> CursorPosition {
        self.owner.state.borrow().current.cursor
    }
    /// Returns the text with each stored paste substituted for its canonical markers.
    ///
    /// Every stored paste gets one literal replacement pass in creation order,
    /// so text a pass inserts is eligible only for later passes.
    #[must_use]
    pub fn get_expanded_text(&self) -> String {
        self.owner.get_expanded_text()
    }
    /// Replaces normalized text, captures changed content, cancels completion and always notifies.
    pub fn set_text(&self, text: &str) {
        self.owner.set_text(text);
    }
    /// Splices normalized text as one undoable edit and cancels completion; empty input has no effect.
    pub fn insert_text_at_cursor(&self, text: &str) {
        self.owner.insert_text_at_cursor(text);
    }
    /// Adds a trimmed nonempty prompt, suppressing the newest duplicate and retaining 100 entries.
    pub fn add_to_history(&self, text: &str) {
        self.owner.add_to_history(text);
    }
    /// Returns a retained border callable.
    #[must_use]
    pub fn border_color(&self) -> BorderColor {
        self.owner.border_color()
    }
    /// Replaces border styling without requesting a frame.
    pub fn set_border_color(&self, color: BorderColor) {
        let old = self.owner.border.replace(color);
        drop(old);
    }
    /// Returns a retained change callable.
    #[must_use]
    pub fn on_change(&self) -> Option<TextCallback> {
        self.owner.on_change()
    }
    /// Replaces or removes change notification.
    pub fn set_on_change(&self, callback: Option<TextCallback>) {
        let old = self.owner.change.replace(callback);
        drop(old);
    }
    /// Returns a retained submission callable.
    #[must_use]
    pub fn on_submit(&self) -> Option<TextCallback> {
        self.owner.on_submit()
    }
    /// Replaces or removes submission notification.
    pub fn set_on_submit(&self, callback: Option<TextCallback>) {
        let old = self.owner.submit.replace(callback);
        drop(old);
    }
    /// Returns whether submission is disabled.
    #[must_use]
    pub fn disable_submit(&self) -> bool {
        self.owner.disabled.get()
    }
    /// Gates submission without changing text.
    pub fn set_disable_submit(&self, disabled: bool) {
        self.owner.disabled.set(disabled);
    }
    /// Replaces the completion provider, cancelling admitted completion and clearing its menu.
    pub fn set_autocomplete_provider(
        &self,
        provider: Rc<dyn AutocompleteProvider<Signal = Cancellation>>,
    ) {
        self.owner.set_provider(provider);
    }
    /// Whether a completion menu is visible, regardless of any running request.
    #[must_use]
    pub fn is_showing_autocomplete(&self) -> bool {
        self.owner.is_showing_autocomplete()
    }
}
impl Owner {
    /// Returns joined logical lines.
    fn get_text(&self) -> String {
        self.state.borrow().current.lines.join("\n")
    }
    /// Returns a retained border callable.
    fn border_color(&self) -> BorderColor {
        self.border.borrow().clone()
    }
    /// Returns a retained change callable.
    fn on_change(&self) -> Option<TextCallback> {
        self.change.borrow().clone()
    }
    /// Returns a retained submission callable.
    fn on_submit(&self) -> Option<TextCallback> {
        self.submit.borrow().clone()
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
        &self.owner.focus
    }
}
impl Component for Editor {
    fn render(&self, width: usize) -> Vec<String> {
        self.owner.draw(width)
    }
    fn input_handler(&self) -> Option<&dyn InputHandler> {
        Some(self)
    }
    fn focusable(&self) -> Option<&dyn Focusable> {
        Some(self)
    }
}
impl InputHandler for Editor {
    fn handle_input(&self, data: &str) {
        self.owner.handle_input(data);
    }
}

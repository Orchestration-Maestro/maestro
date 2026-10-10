#![cfg(test)]
//! A scripted completion provider and an editor wired to the controlled host.
use crate::support::{self, manual_runtime::ManualRuntime};
use maestro_cancellation::Cancellation;
use maestro_tui::autocomplete::{
    CompletionError, CompletionOptions, CompletionResult, CursorPosition,
};
use maestro_tui::tui::TuiRuntime;
use maestro_tui::{
    AutocompleteItem, AutocompleteProvider, AutocompleteSuggestions, Component, Editor,
    EditorOptions, EditorTheme, Focusable, TUI, tui::InputHandler,
};
use std::{
    cell::{Cell, RefCell},
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
    time::Duration,
};

/// The outcome a provider future yields.
pub type Reply = Result<Option<AutocompleteSuggestions>, CompletionError>;

/// What a provider saw in one request.
#[derive(Clone)]
pub struct Call {
    /// Lines passed in.
    pub lines: Vec<String>,
    /// Force option passed in.
    pub force: Option<bool>,
    /// The cancellation signal of the request.
    pub signal: Cancellation,
}

/// Answers a request at once.
type Auto = Box<dyn Fn(&Call) -> Reply>;

/// A held reply slot shared with its pending future.
type Slot = Rc<RefCell<Option<Reply>>>;

/// Suggests from a script and applies by replacing the prefix with the value.
pub struct Scripted {
    /// Requests in arrival order.
    pub calls: RefCell<Vec<Call>>,
    /// Slots of requests awaiting a reply.
    slots: RefCell<Vec<Slot>>,
    /// Answers every request at once when set.
    auto: RefCell<Option<Auto>>,
    /// The opinion on forced file completion.
    pub trigger: Cell<Option<bool>>,
    /// How many times the opinion was asked for.
    pub trigger_asks: Cell<usize>,
    /// Items applied, in order.
    pub applied: RefCell<Vec<String>>,
    /// Runs inside every application, before its result is computed.
    pub on_apply: RefCell<Option<Box<dyn Fn()>>>,
}

/// Waits for a slot to receive its reply.
struct Wait(Slot);

impl Future for Wait {
    type Output = Reply;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Reply> {
        self.0
            .borrow_mut()
            .take()
            .map_or(Poll::Pending, Poll::Ready)
    }
}

/// Something a provider future can yield.
pub trait IntoReply {
    /// The reply it stands for.
    fn into_reply(self) -> Reply;
}

impl IntoReply for Reply {
    fn into_reply(self) -> Reply {
        self
    }
}

impl IntoReply for AutocompleteSuggestions {
    fn into_reply(self) -> Reply {
        Ok(Some(self))
    }
}

impl IntoReply for Option<AutocompleteSuggestions> {
    fn into_reply(self) -> Reply {
        Ok(self)
    }
}

/// A candidate whose value and label are the same text.
pub fn item(value: &str) -> AutocompleteItem {
    AutocompleteItem {
        value: value.to_owned(),
        label: value.to_owned(),
        description: None,
    }
}

/// Suggestions of the given values for a prefix.
pub fn offer(prefix: &str, values: &[&str]) -> AutocompleteSuggestions {
    AutocompleteSuggestions {
        items: values.iter().map(|value| item(value)).collect(),
        prefix: prefix.to_owned(),
    }
}

/// A provider failure carrying a token whose shared allocation identifies the instance.
#[derive(Debug)]
pub struct Marked(pub Rc<()>);

impl std::fmt::Display for Marked {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("marked failure")
    }
}

impl std::error::Error for Marked {}

/// A provider failure that carries `token`.
pub fn failure(token: &Rc<()>) -> Reply {
    Err(Box::new(Marked(Rc::clone(token))))
}

impl Scripted {
    /// A provider that holds every reply until the test supplies it.
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            calls: RefCell::default(),
            slots: RefCell::default(),
            auto: RefCell::default(),
            trigger: Cell::new(None),
            trigger_asks: Cell::new(0),
            applied: RefCell::default(),
            on_apply: RefCell::default(),
        })
    }

    /// Answers every later request immediately with `reply(call)`.
    pub fn answer<R: IntoReply>(&self, reply: impl Fn(&Call) -> R + 'static) {
        self.auto
            .replace(Some(Box::new(move |call| reply(call).into_reply())));
    }

    /// Supplies the reply of request `index` (counting held requests only).
    pub fn resolve(&self, index: usize, reply: impl IntoReply) {
        *self.slots.borrow()[index].borrow_mut() = Some(reply.into_reply());
    }

    /// Supplies the reply of the newest held request.
    pub fn resolve_last(&self, reply: impl IntoReply) {
        let last = self.slots.borrow().len() - 1;
        self.resolve(last, reply);
    }

    /// Requests received so far.
    pub fn count(&self) -> usize {
        self.calls.borrow().len()
    }

    /// Whether request `index` has been aborted.
    pub fn aborted(&self, index: usize) -> bool {
        self.calls.borrow()[index].signal.is_aborted()
    }

    /// Owners of the reply slot of held request `index`.
    pub fn slot_owners(&self, index: usize) -> usize {
        Rc::strong_count(&self.slots.borrow()[index])
    }
}

impl AutocompleteProvider for Scripted {
    type Signal = Cancellation;

    fn get_suggestions<'a>(
        &'a self,
        lines: &'a [String],
        _cursor: CursorPosition,
        options: CompletionOptions<'a, Cancellation>,
    ) -> Pin<Box<dyn Future<Output = Reply> + 'a>> {
        let call = Call {
            lines: lines.to_vec(),
            force: options.force,
            signal: options.signal.clone(),
        };
        let slot = Slot::default();
        if let Some(auto) = &*self.auto.borrow() {
            *slot.borrow_mut() = Some(auto(&call));
        } else {
            self.slots.borrow_mut().push(Rc::clone(&slot));
        }
        self.calls.borrow_mut().push(call);
        Box::pin(Wait(slot))
    }

    fn apply_completion(
        &self,
        lines: &[String],
        cursor: CursorPosition,
        item: &AutocompleteItem,
        prefix: &str,
    ) -> CompletionResult {
        self.applied.borrow_mut().push(item.value.clone());
        if let Some(hook) = &*self.on_apply.borrow() {
            hook();
        }
        let mut lines = lines.to_vec();
        let line = &mut lines[cursor.line];
        let start = cursor.col - prefix.len();
        line.replace_range(start..cursor.col, &item.value);
        CompletionResult {
            lines,
            cursor_line: cursor.line,
            cursor_col: start + item.value.len(),
        }
    }

    fn should_trigger_file_completion(
        &self,
        _lines: &[String],
        _cursor: CursorPosition,
    ) -> Option<bool> {
        self.trigger_asks.set(self.trigger_asks.get() + 1);
        self.trigger.get()
    }
}

/// An editor, its provider and the controlled host.
pub struct Rig {
    /// Keeps default bindings for the test's duration.
    pub _globals: Box<dyn std::any::Any>,
    /// The writer, kept so frame requests stay live.
    pub _tui: TUI,
    /// The controlled host.
    pub runtime: ManualRuntime,
    /// The editor under test.
    pub editor: Rc<Editor>,
    /// The installed provider.
    pub provider: Rc<Scripted>,
    /// Texts the change callback saw.
    pub changes: Rc<RefCell<Vec<String>>>,
}

impl Rig {
    /// A focused editor with a scripted provider and a change log.
    pub fn new() -> Self {
        Self::with_theme(support::theme())
    }

    /// Like [`Rig::new`], with the given theme.
    pub fn with_theme(theme: EditorTheme) -> Self {
        let globals = support::globals();
        let (tui, _, runtime) = support::host(24);
        let editor = Rc::new(Editor::new(&tui, theme, EditorOptions::default()));
        editor.focus_flag().set(true);
        let provider = Scripted::new();
        editor.set_autocomplete_provider(provider.clone());
        let changes = Rc::new(RefCell::new(Vec::new()));
        let log = Rc::clone(&changes);
        editor.set_on_change(Some(Rc::new(move |text| {
            log.borrow_mut().push(text.to_owned());
        })));
        Self {
            _globals: Box::new(globals),
            _tui: tui,
            runtime,
            editor,
            provider,
            changes,
        }
    }

    /// Sends input to the editor.
    pub fn input(&self, data: &str) {
        self.editor.handle_input(data);
    }

    /// Types each character as its own input.
    pub fn typed(&self, text: &str) {
        for scalar in text.chars() {
            self.input(&scalar.to_string());
        }
    }

    /// Runs the futures the editor handed to the host and returns their errors.
    pub fn run(&self) -> Vec<CompletionError> {
        self.runtime.drain_futures()
    }

    /// Moves the clock forward by `ms`, then runs the futures.
    pub fn wait(&self, ms: u64) -> Vec<CompletionError> {
        let target = self.runtime.now() + Duration::from_millis(ms);
        let _ = self.runtime.advance_to(target);
        self.run()
    }

    /// The menu rows of a render at `width`.
    pub fn menu(&self, width: usize) -> Vec<String> {
        let lines = self.editor.render(width);
        lines[self.editor.get_lines().len() + 2..].to_vec()
    }
}

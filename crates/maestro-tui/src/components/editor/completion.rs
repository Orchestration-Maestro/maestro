//! Completion provider ownership, cancellation, delayed starts and serial requests.
use super::{Owner, completion_input::Context};
use crate::autocomplete::{
    AutocompleteItem, AutocompleteProvider, AutocompleteSuggestions, CompletionOptions,
    CursorPosition,
};
use crate::{SelectItem, SelectList, SelectListLayoutOptions, tui::RenderTimer};
use maestro_cancellation::Cancellation;
use std::{rc::Rc, time::Duration};

/// How long attachment and symbol contexts wait for further typing.
const SYMBOL_DEBOUNCE: Duration = Duration::from_millis(20);

/// A provider that observes the editor's own cancellation signal.
pub(super) type Provider = Rc<dyn AutocompleteProvider<Signal = Cancellation>>;

/// Whether completion offers its candidates as a regular or a forced menu.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    /// Candidates for the typed context.
    Regular,
    /// Candidates requested explicitly, kept open while typing.
    Force,
}

/// What asked for a completion request.
#[derive(Clone, Copy)]
pub(super) struct Intent {
    /// Menu mode the result is shown in.
    pub(super) mode: Mode,
    /// Whether an explicit Tab asked, enabling immediate application and skipping the debounce.
    pub(super) tab: bool,
}

/// The visible candidate menu.
pub(super) struct Menu {
    /// Rows rendered below the editor.
    pub(super) list: Rc<SelectList>,
    /// Text the chosen value replaces.
    pub(super) prefix: String,
    /// Mode the menu was requested in.
    pub(super) mode: Mode,
}

/// Provider, menu and request bookkeeping.
#[derive(Default)]
pub(super) struct Completion {
    /// Installed provider.
    provider: Option<Provider>,
    /// Visible menu.
    pub(super) menu: Option<Menu>,
    /// Identifies the newest admissible start; cancellation advances it.
    token: u64,
    /// Identifies the newest request that began running.
    request: u64,
    /// Signal of the newest running request.
    abort: Option<Cancellation>,
    /// Pending debounced start.
    timer: Option<Box<dyn RenderTimer>>,
    /// Whether a request future is in flight.
    running: bool,
    /// Newest start waiting for the running request to settle.
    queued: Option<(u64, Intent)>,
}

/// Owned inputs of one running request.
struct Request {
    /// Provider captured at start.
    provider: Provider,
    /// Cancellation observed by the provider.
    signal: Cancellation,
    /// Identity among running requests.
    id: u64,
    /// Lines at start.
    lines: Vec<String>,
    /// Byte cursor at start.
    cursor: CursorPosition,
}

impl Owner {
    /// Installs a provider after cancelling admitted completion and clearing the menu.
    pub(super) fn set_provider(&self, provider: Provider) {
        self.cancel_autocomplete();
        let old = self.completion.borrow_mut().provider.replace(provider);
        drop(old);
    }

    /// The installed provider.
    pub(super) fn provider(&self) -> Option<Provider> {
        self.completion.borrow().provider.clone()
    }

    /// Whether a menu is visible.
    pub(super) fn is_showing_autocomplete(&self) -> bool {
        self.completion.borrow().menu.is_some()
    }

    /// Invalidates scheduled and running requests without touching the menu.
    pub(super) fn cancel_request(&self) {
        let (timer, abort) = {
            let mut completion = self.completion.borrow_mut();
            completion.token += 1;
            (completion.timer.take(), completion.abort.take())
        };
        if let Some(mut timer) = timer {
            timer.cancel();
        }
        if let Some(abort) = abort {
            abort.abort();
        }
    }

    /// Invalidates requests and removes the menu.
    pub(super) fn cancel_autocomplete(&self) {
        self.cancel_request();
        let old = self.completion.borrow_mut().menu.take();
        drop(old);
    }

    /// Refreshes the visible menu in its own mode.
    pub(super) fn update_autocomplete(&self) {
        let mode = self.completion.borrow().menu.as_ref().map(|menu| menu.mode);
        if let Some(mode) = mode {
            self.request_completion(Intent { mode, tab: false });
        }
    }

    /// Cancels earlier work, then starts a request now or after its debounce.
    pub(super) fn request_completion(&self, intent: Intent) {
        let Some(provider) = self.provider() else {
            return;
        };
        if intent.mode == Mode::Force {
            let (lines, cursor) = self.input();
            if provider.should_trigger_file_completion(&lines, cursor) == Some(false) {
                return;
            }
        }
        self.cancel_request();
        let token = self.completion.borrow().token;
        let delay = self.debounce(intent);
        if delay.is_zero() {
            self.admit(token, intent);
            return;
        }
        let weak = self.me.clone();
        let timer = self.runtime.schedule(
            delay,
            Box::new(move || {
                if let Some(owner) = weak.upgrade() {
                    let old = owner.completion.borrow_mut().timer.take();
                    drop(old);
                    owner.admit(token, intent);
                }
                Ok(())
            }),
        );
        self.completion.borrow_mut().timer = Some(timer);
    }

    /// Delay before a request starts.
    fn debounce(&self, intent: Intent) -> Duration {
        if intent.tab
            || intent.mode == Mode::Force
            || !self.before_cursor_matches(Context::Debounce)
        {
            Duration::ZERO
        } else {
            SYMBOL_DEBOUNCE
        }
    }

    /// Starts the request, or keeps it as the newest start behind a running one.
    fn admit(&self, token: u64, intent: Intent) {
        let start = {
            let mut completion = self.completion.borrow_mut();
            if completion.running {
                completion.queued = Some((token, intent));
                false
            } else {
                completion.running = true;
                true
            }
        };
        if start {
            self.spawn_request(token, intent);
        }
    }

    /// Hands one request future to the host.
    fn spawn_request(&self, token: u64, intent: Intent) {
        let weak = self.me.clone();
        self.runtime.spawn_local(Box::pin(async move {
            let Some(owner) = weak.upgrade() else {
                return Ok(());
            };
            let Some(request) = owner.begin(token) else {
                owner.settle();
                return Ok(());
            };
            drop(owner);
            let options = CompletionOptions {
                signal: &request.signal,
                force: Some(intent.mode == Mode::Force),
            };
            let outcome = request
                .provider
                .get_suggestions(&request.lines, request.cursor, options)
                .await;
            let Some(owner) = weak.upgrade() else {
                return outcome.map(drop);
            };
            let result = outcome.map(|suggestions| owner.finish(&request, suggestions, intent));
            owner.settle();
            result
        }));
    }

    /// Captures a request when its start is still admissible.
    fn begin(&self, token: u64) -> Option<Request> {
        let (provider, signal) = {
            let mut completion = self.completion.borrow_mut();
            if completion.token != token {
                return None;
            }
            let provider = completion.provider.clone()?;
            let signal = Cancellation::new();
            completion.request += 1;
            completion.abort = Some(signal.clone());
            (provider, signal)
        };
        let (lines, cursor) = self.input();
        let id = self.completion.borrow().request;
        Some(Request {
            provider,
            signal,
            id,
            lines,
            cursor,
        })
    }

    /// Releases the serial slot and starts the newest admissible queued start.
    fn settle(&self) {
        let next = {
            let mut completion = self.completion.borrow_mut();
            let token = completion.token;
            let next = completion
                .queued
                .take()
                .filter(|&(queued, _)| queued == token);
            completion.running = next.is_some();
            next
        };
        if let Some((token, intent)) = next {
            self.spawn_request(token, intent);
        }
    }

    /// Whether live editor state and cancellation still admit the request's result.
    fn current(&self, request: &Request) -> bool {
        if request.signal.is_aborted() || request.id != self.completion.borrow().request {
            return false;
        }
        let state = self.state.borrow();
        request.cursor == state.current.cursor && request.lines == state.current.lines
    }

    /// Applies a current result: nothing, an immediate edit or a menu.
    fn finish(&self, request: &Request, result: Option<AutocompleteSuggestions>, intent: Intent) {
        if !self.current(request) {
            return;
        }
        let old = self.completion.borrow_mut().abort.take();
        drop(old);
        let Some(suggestions) = result.filter(|suggestions| !suggestions.items.is_empty()) else {
            self.cancel_autocomplete();
            (self.request)();
            return;
        };
        if intent.mode == Mode::Force && intent.tab && suggestions.items.len() == 1 {
            self.apply_item(
                &request.provider,
                &suggestions.items[0],
                &suggestions.prefix,
            );
            self.notify();
        } else {
            self.show_menu(suggestions, intent.mode);
        }
        (self.request)();
    }

    /// Replaces the menu with one built from the suggestions.
    fn show_menu(&self, suggestions: AutocompleteSuggestions, mode: Mode) {
        let layout = if suggestions.prefix.starts_with('/') {
            SelectListLayoutOptions {
                min_primary_column_width: Some(12),
                max_primary_column_width: Some(32),
                truncate_primary: None,
            }
        } else {
            SelectListLayoutOptions::default()
        };
        let best = best_match(&suggestions.items, &suggestions.prefix);
        let items = suggestions.items.into_iter().map(|item| SelectItem {
            value: item.value,
            label: item.label,
            description: item.description,
        });
        let list = SelectList::new(
            items.collect(),
            self.maximum.get(),
            self.theme.select_list.clone(),
            layout,
        );
        if let Some(index) = best {
            list.set_selected_index(isize::try_from(index).unwrap_or(isize::MAX));
        }
        let menu = Menu {
            list: Rc::new(list),
            prefix: suggestions.prefix,
            mode,
        };
        let old = self.completion.borrow_mut().menu.replace(menu);
        drop(old);
    }

    /// Commits the provider's applied text and cursor as one undoable edit.
    pub(super) fn apply_item(&self, provider: &Provider, item: &AutocompleteItem, prefix: &str) {
        let (lines, cursor) = {
            let mut state = self.state.borrow_mut();
            state.snapshot();
            state.reset_action();
            (state.current.lines.clone(), state.current.cursor)
        };
        let result = provider.apply_completion(&lines, cursor, item, prefix);
        let mut state = self.state.borrow_mut();
        state.current.lines = result.lines;
        state.current.cursor.line = result.cursor_line;
        state.set_col(result.cursor_col);
    }

    /// Owned copy of the lines and byte cursor.
    fn input(&self) -> (Vec<String>, CursorPosition) {
        let state = self.state.borrow();
        (state.current.lines.clone(), state.current.cursor)
    }
}

/// Index of the exact value match, else of the first value that starts with the prefix.
fn best_match(items: &[AutocompleteItem], prefix: &str) -> Option<usize> {
    if prefix.is_empty() {
        return None;
    }
    items
        .iter()
        .position(|item| item.value == prefix)
        .or_else(|| items.iter().position(|item| item.value.starts_with(prefix)))
}

//! Menu key routing, selected value application, context classification and menu rows.
use super::Owner;
use super::completion::{Intent, Mode};
use crate::autocomplete::AutocompleteItem;
use crate::text::utils::is_whitespace_scalar;
use crate::{KeybindingsManager, visible_width};
use regress::Regex;
use std::sync::LazyLock;

/// Attachment contexts that wait for further typing: an `@` token (quoted or not) or a `#` token.
static DEBOUNCE: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r#"(?:^|[ \t])(?:@(?:"[^"]*|[^\s]*)|#[^\s]*)$"#).ok());
/// An `@` or `#` token at the end of the text before the cursor.
static SYMBOL: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"(?:^|[\s])[@#][^\s]*$").ok());

/// Which text-before-cursor pattern to test.
#[derive(Clone, Copy)]
pub(super) enum Context {
    /// Attachment tokens that debounce.
    Debounce,
    /// Symbol tokens that automatically trigger.
    Symbol,
}

impl Owner {
    /// Handles a key while a menu is open; false lets ordinary input dispatch continue.
    pub(super) fn menu_input(&self, data: &str, bindings: &KeybindingsManager) -> bool {
        let menu = {
            let completion = self.completion.borrow();
            completion
                .menu
                .as_ref()
                .map(|menu| (menu.list.clone(), menu.prefix.clone()))
        };
        let Some((list, prefix)) = menu else {
            return false;
        };
        if bindings.matches(data, "tui.select.cancel") {
            self.cancel_autocomplete();
            return true;
        }
        if bindings.matches(data, "tui.select.up") || bindings.matches(data, "tui.select.down") {
            crate::tui::InputHandler::handle_input(&*list, data);
            return true;
        }
        let tab = bindings.matches(data, "tui.input.tab");
        if !tab && !bindings.matches(data, "tui.select.confirm") {
            return false;
        }
        let (Some(selected), Some(provider)) = (list.get_selected_item(), self.provider()) else {
            return tab;
        };
        let item = AutocompleteItem {
            value: selected.value.clone(),
            label: selected.label.clone(),
            description: selected.description.clone(),
        };
        self.apply_item(&provider, &item, &prefix);
        self.cancel_autocomplete();
        if !tab && prefix.starts_with('/') {
            return false;
        }
        self.notify();
        true
    }

    /// Explicit Tab: regular completion for a bare slash command, forced otherwise.
    pub(super) fn tab_completion(&self) {
        if self.provider().is_none() {
            return;
        }
        let before = self.before_cursor();
        let command = self.in_slash_context(&before)
            && !before
                .trim_start_matches(is_whitespace_scalar)
                .contains(' ');
        let mode = if command { Mode::Regular } else { Mode::Force };
        self.request_completion(Intent { mode, tab: true });
    }

    /// Refreshes the menu or starts completion for a typed chunk.
    pub(super) fn complete_typed(&self, text: &str) {
        if self.is_showing_autocomplete() {
            self.update_autocomplete();
            return;
        }
        let before = self.before_cursor();
        let triggers = match text {
            "/" => self.at_start_of_message(&before),
            "@" | "#" => before.len() == 1 || before[..before.len() - 1].ends_with([' ', '\t']),
            _ => {
                text.chars()
                    .any(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
                    && (self.in_slash_context(&before) || matches(Context::Symbol, &before))
            }
        };
        if triggers {
            self.request_completion(Intent {
                mode: Mode::Regular,
                tab: false,
            });
        }
    }

    /// Refreshes the menu or retriggers completion after a deletion.
    pub(super) fn complete_deleted(&self) {
        if self.is_showing_autocomplete() {
            self.update_autocomplete();
            return;
        }
        let before = self.before_cursor();
        if self.in_slash_context(&before) || matches(Context::Symbol, &before) {
            self.request_completion(Intent {
                mode: Mode::Regular,
                tab: false,
            });
        }
    }

    /// Whether the text before the cursor matches a pattern.
    pub(super) fn before_cursor_matches(&self, context: Context) -> bool {
        matches(context, &self.before_cursor())
    }

    /// The current line up to the cursor.
    fn before_cursor(&self) -> String {
        let state = self.state.borrow();
        let cursor = state.current.cursor;
        state.current.lines[cursor.line][..cursor.col].to_owned()
    }

    /// Whether a slash menu is allowed and the text starts a command.
    fn in_slash_context(&self, before: &str) -> bool {
        self.state.borrow().current.cursor.line == 0
            && before
                .trim_start_matches(is_whitespace_scalar)
                .starts_with('/')
    }

    /// Whether the text before the cursor on the first line is empty or one slash.
    fn at_start_of_message(&self, before: &str) -> bool {
        let text = before.trim_matches(is_whitespace_scalar);
        self.state.borrow().current.cursor.line == 0 && (text.is_empty() || text == "/")
    }

    /// Rows of the open menu inside the editor's side padding.
    pub(super) fn menu_rows(&self, width: usize, padding: usize) -> Vec<String> {
        let list = self
            .completion
            .borrow()
            .menu
            .as_ref()
            .map(|menu| menu.list.clone());
        let Some(list) = list else {
            return Vec::new();
        };
        let side = " ".repeat(padding);
        crate::Component::render(&*list, width)
            .into_iter()
            .map(|row| {
                let fill = " ".repeat(width.saturating_sub(visible_width(&row)));
                format!("{side}{row}{fill}{side}")
            })
            .collect()
    }
}

/// Whether the pattern matches the end of `before`.
fn matches(context: Context, before: &str) -> bool {
    let regex = match context {
        Context::Debounce => &DEBOUNCE,
        Context::Symbol => &SYMBOL,
    };
    regex
        .as_ref()
        .is_some_and(|regex| regex.find(before).is_some())
}

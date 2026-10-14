//! Earlier-message selection retaining chronological input order.
use super::dynamic_border::DynamicBorder;
use maestro_theme::{LiveTheme, Theme, ThemeColor, paint};
use maestro_tui::tui::{InputHandler, TuiRuntime};
use maestro_tui::{Component, Container, Spacer, Text, TruncateOptions, truncate_to_width};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// A selectable session entry.
pub struct UserMessageItem {
    /// Entry identity reported on confirmation.
    pub id: String,
    /// Message text displayed in the list.
    pub text: String,
    /// Optional timestamp accepted without displaying it.
    pub timestamp: Option<String>,
}
/// Constructor selection and supplied presentation effects.
pub struct UserMessageSelectorOptions {
    /// Initial identity; empty or unmatched identities select newest.
    pub initial_selected_id: Option<String>,
    /// Published theme used for live rows.
    pub theme: LiveTheme,
    /// Host used for empty-list deferred cancellation.
    pub runtime: Rc<dyn TuiRuntime>,
}
/// Entry selection notification.
type Selection = Rc<dyn Fn(&str)>;
/// A focusable chronological list with mutable callback slots.
pub struct UserMessageList {
    /// Owned chronological entries.
    messages: Vec<UserMessageItem>,
    /// Selected position.
    selected: Cell<usize>,
    /// Confirmation notification.
    select: RefCell<Option<Selection>>,
    /// Cancellation notification.
    cancel: RefCell<Option<Rc<dyn Fn()>>>,
    /// Live row colors.
    theme: LiveTheme,
}
impl UserMessageList {
    /// Owns records, selecting the first supplied identity or newest entry.
    fn new(messages: Vec<UserMessageItem>, initial: Option<&str>, theme: LiveTheme) -> Self {
        let selected = initial
            .filter(|id| !id.is_empty())
            .and_then(|id| messages.iter().position(|message| message.id == id))
            .unwrap_or_else(|| messages.len().saturating_sub(1));
        Self {
            messages,
            selected: Cell::new(selected),
            select: RefCell::default(),
            cancel: RefCell::default(),
            theme,
        }
    }
    /// Returns the current confirmation handle.
    #[must_use]
    pub fn on_select(&self) -> Option<Selection> {
        self.select.borrow().clone()
    }
    /// Replaces or removes confirmation notification.
    pub fn set_on_select(&self, callback: Option<Selection>) {
        let old = self.select.replace(callback);
        drop(old);
    }
    /// Returns the current cancellation handle.
    #[must_use]
    pub fn on_cancel(&self) -> Option<Rc<dyn Fn()>> {
        self.cancel.borrow().clone()
    }
    /// Replaces or removes cancellation notification.
    pub fn set_on_cancel(&self, callback: Option<Rc<dyn Fn()>>) {
        let old = self.cancel.replace(callback);
        drop(old);
    }
    /// Samples the currently published foreground.
    fn paint(&self, key: &ThemeColor, text: &str) -> String {
        paint(self.theme.get().ok().as_deref(), key, text)
    }
    /// Composes one message's text, metadata and separator.
    fn rows(&self, index: usize, width: usize) -> [String; 3] {
        let normalized = self.messages[index].text.replace('\n', " ");
        let clipped = truncate_to_width(
            normalized.trim_matches(maestro_app::is_whitespace_scalar),
            width.saturating_sub(2),
            TruncateOptions::default(),
        );
        let text = if index == self.selected.get() {
            format!(
                "{}{}",
                self.paint(&ThemeColor::Accent, "› "),
                Theme::bold(&clipped, true)
            )
        } else {
            format!("  {clipped}")
        };
        [
            text,
            self.paint(
                &ThemeColor::Muted,
                &format!("  Message {} of {}", index + 1, self.messages.len()),
            ),
            String::new(),
        ]
    }
}
impl Component for UserMessageList {
    fn render(&self, width: usize) -> Vec<String> {
        let count = self.messages.len();
        if count == 0 {
            return vec![self.paint(&ThemeColor::Muted, "  No user messages found")];
        }
        let start = self
            .selected
            .get()
            .saturating_sub(5)
            .min(count.saturating_sub(10));
        let end = (start + 10).min(count);
        let mut lines: Vec<_> = (start..end)
            .flat_map(|index| self.rows(index, width))
            .collect();
        if start > 0 || end < count {
            lines.push(self.paint(
                &ThemeColor::Muted,
                &format!("  ({}/{count})", self.selected.get() + 1),
            ));
        }
        lines
    }
    fn input_handler(&self) -> Option<&dyn InputHandler> {
        Some(self)
    }
}
impl InputHandler for UserMessageList {
    fn handle_input(&self, data: &str) {
        let bindings = maestro_tui::get_keybindings();
        let last = self.messages.len().saturating_sub(1);
        if bindings.matches(data, "tui.select.up") {
            self.selected.set(if self.selected.get() == 0 {
                last
            } else {
                self.selected.get() - 1
            });
        } else if bindings.matches(data, "tui.select.down") {
            self.selected.set(if self.selected.get() >= last {
                0
            } else {
                self.selected.get() + 1
            });
        } else if bindings.matches(data, "tui.select.confirm") {
            if let (Some(message), Some(callback)) =
                (self.messages.get(self.selected.get()), self.on_select())
            {
                callback(&message.id);
            }
        } else if bindings.matches(data, "tui.select.cancel")
            && let Some(callback) = self.on_cancel()
        {
            callback();
        }
    }
}
/// A message-list panel whose focus handle is independent of its chrome.
pub struct UserMessageSelectorComponent {
    /// Shared child-array owner.
    container: Container,
    /// Retained focus target.
    message_list: Rc<UserMessageList>,
}
impl UserMessageSelectorComponent {
    /// Builds the message panel from supplied records and presentation effects.
    #[must_use]
    pub fn new(
        messages: Vec<UserMessageItem>,
        on_select: Selection,
        on_cancel: Rc<dyn Fn()>,
        options: UserMessageSelectorOptions,
    ) -> Self {
        let container = Container::new();
        let theme = options.theme;
        container.add_child(Rc::new(Spacer::new(1)));
        container.add_child(Rc::new(Text::new(
            Theme::bold("Fork from Message", true),
            1,
            0,
            None,
        )));
        container.add_child(Rc::new(Text::new(
            paint(
                theme.get().ok().as_deref(),
                &ThemeColor::Muted,
                "Select a user message to copy the active path up to that point into a new session",
            ),
            1,
            0,
            None,
        )));
        container.add_child(Rc::new(Spacer::new(1)));
        container.add_child(Rc::new(DynamicBorder::new(theme.clone(), None)));
        container.add_child(Rc::new(Spacer::new(1)));
        let message_list = Rc::new(UserMessageList::new(
            messages,
            options.initial_selected_id.as_deref(),
            theme.clone(),
        ));
        message_list.set_on_select(Some(on_select));
        message_list.set_on_cancel(Some(on_cancel.clone()));
        container.add_child(message_list.clone());
        container.add_child(Rc::new(Spacer::new(1)));
        container.add_child(Rc::new(DynamicBorder::new(theme, None)));
        if message_list.messages.is_empty() {
            options.runtime.schedule(
                std::time::Duration::from_millis(100),
                Box::new(move || {
                    on_cancel();
                    Ok(())
                }),
            );
        }
        Self {
            container,
            message_list,
        }
    }
    /// Returns the retained focus target.
    #[must_use]
    pub fn get_message_list(&self) -> &Rc<UserMessageList> {
        &self.message_list
    }
}
component_children!(UserMessageSelectorComponent, Container, container);
impl Component for UserMessageSelectorComponent {
    fn render(&self, width: usize) -> Vec<String> {
        self.container.render(width)
    }
    fn invalidate(&self) {
        self.container.invalidate();
    }
}

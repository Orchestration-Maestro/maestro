//! Live session scope, membership and ordering chrome.
use crate::{NameFilter, SortMode};
use maestro_theme::{LiveTheme, Theme, ThemeColor, paint};
use maestro_tui::interface::{RenderTimer, TuiRuntime};
use maestro_tui::{Component, TruncateOptions, truncate_to_width, visible_width};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};
/// Session discovery scope.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SessionScope {
    /// Sessions from the current working directory.
    Current,
    /// Sessions from all working directories.
    All,
}
/// Caller-owned status text.
pub enum StatusMessage {
    /// Informational status.
    Info(String),
    /// Error status.
    Error(String),
}
/// Presentation and deferred rendering effects.
pub struct SessionSelectorHeaderOptions {
    /// Current published colors.
    pub theme: LiveTheme,
    /// Scheduling owner.
    pub runtime: Rc<dyn TuiRuntime>,
    /// Redraw notification.
    pub request_render: Rc<dyn Fn()>,
    /// Live bold decoration policy.
    pub styles_enabled: Rc<dyn Fn() -> bool>,
}
/// Session discovery chrome with caller-controlled state.
pub struct SessionSelectorHeader {
    /// Discovery scope.
    scope: Cell<SessionScope>,
    /// Ordering policy.
    sort_mode: Cell<SortMode>,
    /// Membership policy.
    name_filter: Cell<NameFilter>,
    /// Whether discovery is loading.
    loading: Cell<bool>,
    /// Progress belonging to this load.
    progress: Cell<Option<(usize, usize)>>,
    /// Authored path display policy.
    show_path: Cell<bool>,
    /// Rename help visibility.
    show_rename_hint: Cell<bool>,
    /// Delete confirmation, including a present empty path.
    confirming: RefCell<Option<String>>,
    /// Caller status payload.
    status: RefCell<Option<StatusMessage>>,
    /// Cancelled on replacement and teardown.
    timer: RefCell<Option<Box<dyn RenderTimer>>>,
    /// Retained presentation effects.
    options: SessionSelectorHeaderOptions,
}
impl SessionSelectorHeader {
    /// Retains independent constructor policies and presentation effects.
    #[must_use]
    pub fn new(
        scope: SessionScope,
        sort_mode: SortMode,
        name_filter: NameFilter,
        options: SessionSelectorHeaderOptions,
    ) -> Rc<Self> {
        Rc::new(Self {
            show_path: Cell::new(false),
            show_rename_hint: Cell::new(false),
            confirming: RefCell::default(),
            status: RefCell::default(),
            timer: RefCell::default(),
            loading: Cell::new(false),
            progress: Cell::new(None),
            scope: Cell::new(scope),
            sort_mode: Cell::new(sort_mode),
            name_filter: Cell::new(name_filter),
            options,
        })
    }
    /// Sets discovery scope without requesting a redraw.
    pub fn set_scope(&self, scope: SessionScope) {
        self.scope.set(scope);
    }
    /// Sets ordering without requesting a redraw.
    pub fn set_sort_mode(&self, mode: SortMode) {
        self.sort_mode.set(mode);
    }
    /// Sets name membership without requesting a redraw.
    pub fn set_name_filter(&self, filter: NameFilter) {
        self.name_filter.set(filter);
    }
    /// Sets the authored-path hint state.
    pub fn set_show_path(&self, show: bool) {
        self.show_path.set(show);
    }
    /// Sets rename help visibility.
    pub fn set_show_rename_hint(&self, show: bool) {
        self.show_rename_hint.set(show);
    }
    /// Sets delete confirmation presence.
    pub fn set_confirming_delete_path(&self, path: Option<String>) {
        self.confirming.replace(path);
    }
    /// Replaces status and its optional nonzero expiry.
    pub fn set_status_message(
        self: &Rc<Self>,
        message: Option<StatusMessage>,
        auto_hide: Option<Duration>,
    ) {
        self.cancel_timer();
        let scheduled = message.is_some();
        self.status.replace(message);
        let Some(delay) = auto_hide.filter(|delay| scheduled && !delay.is_zero()) else {
            return;
        };
        let weak = Rc::downgrade(self);
        let timer = self.options.runtime.schedule(
            delay,
            Box::new(move || {
                if let Some(header) = weak.upgrade() {
                    header.expire();
                }
                Ok(())
            }),
        );
        self.timer.replace(Some(timer));
    }
    /// Clears the expired status before notifying the caller.
    fn expire(&self) {
        self.status.take();
        self.timer.take();
        (self.options.request_render)();
    }
    /// Revokes scheduled delivery outside the state borrow.
    fn cancel_timer(&self) {
        if let Some(mut timer) = self.timer.take() {
            timer.cancel();
        }
    }
    /// Changes loading and clears any prior progress.
    pub fn set_loading(&self, loading: bool) {
        self.loading.set(loading);
        self.progress.set(None);
    }
    /// Reports the current discovery counts.
    pub fn set_progress(&self, loaded: usize, total: usize) {
        self.progress.set(Some((loaded, total)));
    }
    /// Samples the current foreground.
    fn paint(&self, key: &ThemeColor, text: &str) -> String {
        paint(self.options.theme.get().ok().as_deref(), key, text)
    }
    /// Frames a live key hint using its existing owner.
    fn hint(&self, key: &str, text: &str) -> String {
        maestro_app::key_hint(key, text, &self.options.theme)
            .unwrap_or_else(|_| format!("{} {text}", maestro_app::key_text(key)))
    }
    /// Gives confirmation priority over status and ordinary help.
    fn hints(&self, width: usize) -> (String, String) {
        if self.confirming.borrow().is_some() {
            let text = format!(
                "Delete session? {} · {}",
                self.hint("tui.select.confirm", "confirm"),
                self.hint("tui.select.cancel", "cancel")
            );
            return (
                self.paint(&ThemeColor::Error, &clip(&text, width, "…")),
                String::new(),
            );
        }
        let status = self.status.borrow().as_ref().map(|s| match s {
            StatusMessage::Info(s) => (ThemeColor::Accent, clip(s, width, "…")),
            StatusMessage::Error(s) => (ThemeColor::Error, clip(s, width, "…")),
        });
        if let Some((color, text)) = status {
            return (self.paint(&color, &text), String::new());
        }
        let sep = self.paint(&ThemeColor::Muted, " · ");
        let hint1 = self.hint("tui.input.tab", "scope")
            + &sep
            + &self.paint(&ThemeColor::Muted, "re:<pattern> regex · \"phrase\" exact");
        let path = if self.show_path.get() {
            "path (on)"
        } else {
            "path (off)"
        };
        let mut parts = vec![
            self.hint("app.session.toggleSort", "sort"),
            self.hint("app.session.toggleNamedFilter", "named"),
            self.hint("app.session.delete", "delete"),
            self.hint("app.session.togglePath", path),
        ];
        if self.show_rename_hint.get() {
            parts.push(self.hint("app.session.rename", "rename"));
        }
        (
            clip(&hint1, width, "…"),
            clip(&parts.join(&sep), width, "…"),
        )
    }
    /// Composes scope, membership and ordering in the right column.
    fn right(&self) -> String {
        let scope = if self.loading.get() {
            let progress = self.progress.get().map_or_else(
                || "...".into(),
                |(loaded, total)| format!("{loaded}/{total}"),
            );
            self.paint(&ThemeColor::Muted, "○ Current Folder | ")
                + &self.paint(&ThemeColor::Accent, &format!("Loading {progress}"))
        } else {
            match self.scope.get() {
                SessionScope::Current => {
                    self.paint(&ThemeColor::Accent, "◉ Current Folder")
                        + &self.paint(&ThemeColor::Muted, " | ○ All")
                }
                SessionScope::All => {
                    self.paint(&ThemeColor::Muted, "○ Current Folder | ")
                        + &self.paint(&ThemeColor::Accent, "◉ All")
                }
            }
        };
        let name = if self.name_filter.get() == NameFilter::All {
            "All"
        } else {
            "Named"
        };
        let sort = match self.sort_mode.get() {
            SortMode::Threaded => "Threaded",
            SortMode::Recent => "Recent",
            SortMode::Relevance => "Fuzzy",
        };
        format!(
            "{scope}  {}{}  {}{}",
            self.paint(&ThemeColor::Muted, "Name: "),
            self.paint(&ThemeColor::Accent, name),
            self.paint(&ThemeColor::Muted, "Sort: "),
            self.paint(&ThemeColor::Accent, sort)
        )
    }
}
/// Clips styled text with the supplied ellipsis.
fn clip(text: &str, width: usize, ellipsis: &str) -> String {
    truncate_to_width(
        text,
        width,
        TruncateOptions {
            ellipsis,
            ..Default::default()
        },
    )
}
impl Component for SessionSelectorHeader {
    fn render(&self, width: usize) -> Vec<String> {
        let title = if self.scope.get() == SessionScope::Current {
            "Resume Session (Current Folder)"
        } else {
            "Resume Session (All)"
        };
        let right = clip(&self.right(), width, "");
        let left = clip(
            &Theme::bold(title, (self.options.styles_enabled)()),
            width.saturating_sub(visible_width(&right) + 1),
            "",
        );
        let spacing = width.saturating_sub(visible_width(&left) + visible_width(&right));
        let (hint1, hint2) = self.hints(width);
        vec![
            format!("{left}{}{right}", " ".repeat(spacing)),
            hint1,
            hint2,
        ]
    }
}

impl Drop for SessionSelectorHeader {
    fn drop(&mut self) {
        self.cancel_timer();
    }
}

//! Searchable session records with live row presentation.
use super::tree::{FlatSessionNode, build_session_tree};
use crate::components::session_selector_search::filter_session_records;
use crate::{NameFilter, SortMode};
use maestro_app::{SessionInfo, canonicalize_path, is_whitespace_scalar};
use maestro_theme::{LiveTheme, Theme, ThemeBg, ThemeColor, paint};
use maestro_tui::tui::{FocusFlag, Focusable, InputHandler};
use maestro_tui::{Component, Input, TruncateOptions, truncate_to_width, visible_width};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
mod controls;
mod mutations;
use controls::Controls;
use mutations::Mutations;
/// Constructor policies and supplied live observations.
pub struct SessionListOptions {
    /// Existing owner for asynchronous mutation work.
    pub runtime: Rc<dyn maestro_tui::tui::TuiRuntime>,
    /// Show each record's working directory.
    pub show_cwd: bool,
    /// Initial ordering policy.
    pub sort_mode: SortMode,
    /// Initial name membership policy.
    pub name_filter: NameFilter,
    /// Current authored session identity.
    pub current_session_file_path: Option<String>,
    /// Published row palette.
    pub theme: LiveTheme,
    /// Live epoch milliseconds.
    pub now_ms: Rc<dyn Fn() -> f64>,
    /// Live home spelling.
    pub home: Rc<dyn Fn() -> String>,
    /// Live bold decoration policy.
    pub styles_enabled: Rc<dyn Fn() -> bool>,
}
/// Mutable confirmation slot retaining callable identity.
type Selection = Rc<dyn Fn(&str)>;
/// Original session records and their selected display position.
pub struct SessionList {
    /// Owned immutable record snapshot.
    sessions: RefCell<Rc<Vec<Rc<SessionInfo>>>>,
    /// Original occurrences in visible order.
    visible: RefCell<Rc<Vec<FlatSessionNode>>>,
    /// Live membership policy.
    name_filter: Cell<NameFilter>,
    /// Live ordering policy.
    sort_mode: Cell<SortMode>,
    /// Working-directory display policy.
    show_cwd: Cell<bool>,
    /// Authored-path display policy.
    show_path: Cell<bool>,
    /// Delete confirmation display identity.
    confirming: RefCell<Option<String>>,
    /// Canonical current identity captured once.
    current: Option<String>,
    /// Shared confirmation slot.
    select: RefCell<Option<Selection>>,
    /// Shared cancellation slot.
    cancel: RefCell<Option<Rc<dyn Fn()>>>,
    /// Ordered controls and retained named-filter manager.
    controls: Controls,
    /// Replaceable mutation notifications.
    mutations: Mutations,
    /// Independent parent focus forwarding to Input.
    focus: FocusFlag,
    /// Selected visible position.
    selected: Cell<usize>,
    /// Existing editable search owner.
    input: Rc<Input>,
    /// Presentation effects.
    options: SessionListOptions,
}
impl SessionList {
    /// Retains original records and supplied observations.
    #[must_use]
    pub fn new(sessions: Vec<SessionInfo>, options: SessionListOptions) -> Rc<Self> {
        let current = options
            .current_session_file_path
            .as_deref()
            .filter(|p| !p.is_empty())
            .map(canonicalize_path);
        let input = Rc::new(Input::new());
        let list = Rc::new(Self {
            visible: RefCell::default(),
            name_filter: Cell::new(options.name_filter),
            sort_mode: Cell::new(options.sort_mode),
            show_cwd: Cell::new(options.show_cwd),
            show_path: Cell::new(false),
            confirming: RefCell::default(),
            current,
            select: RefCell::default(),
            cancel: RefCell::default(),
            controls: Controls::new(),
            mutations: Mutations::default(),
            focus: FocusFlag::forwarding(input.clone()),
            sessions: RefCell::new(Rc::new(sessions.into_iter().map(Rc::new).collect())),
            selected: Cell::new(0),
            input,
            options,
        });
        list.filter_sessions();
        let weak = Rc::downgrade(&list);
        list.input.set_on_submit(Some(Rc::new(move |_| {
            if let Some(list) = weak.upgrade() {
                list.submit();
            }
        })));
        list
    }
    /// Returns the selected authored path.
    #[must_use]
    pub fn get_selected_session_path(&self) -> Option<String> {
        let sessions = self.sessions.borrow().clone();
        self.visible
            .borrow()
            .get(self.selected.get())
            .map(|node| sessions[node.index].path.clone())
    }
    /// Sets ordering and reapplies the retained query.
    pub fn set_sort_mode(&self, mode: SortMode) {
        self.sort_mode.set(mode);
        self.filter_sessions();
    }
    /// Sets membership and reapplies the retained query.
    pub fn set_name_filter(&self, filter: NameFilter) {
        self.name_filter.set(filter);
        self.filter_sessions();
    }
    /// Replaces records and working-directory display without clearing search.
    pub fn set_sessions(&self, sessions: Vec<SessionInfo>, show_cwd: bool) {
        self.install_sessions(
            Rc::new(sessions.into_iter().map(Rc::new).collect()),
            show_cwd,
        );
    }
    /// Installs an immutable shared discovery snapshot.
    pub(super) fn install_sessions(&self, sessions: Rc<Vec<Rc<SessionInfo>>>, show_cwd: bool) {
        let old = self.sessions.replace(sessions);
        drop(old);
        self.show_cwd.set(show_cwd);
        self.filter_sessions();
    }
    /// Retains the caller's live named-filter manager.
    pub(super) fn configure_keybindings(&self, manager: maestro_tui::KeybindingsManager) {
        self.controls.keybindings.replace(manager);
    }
    /// Returns the current confirmation handle.
    #[must_use]
    pub fn on_select(&self) -> Option<Selection> {
        self.select.borrow().clone()
    }
    /// Replaces or removes confirmation delivery.
    pub fn set_on_select(&self, callback: Option<Selection>) {
        let old = self.select.replace(callback);
        drop(old);
    }
    /// Returns the current cancellation handle.
    #[must_use]
    pub fn on_cancel(&self) -> Option<Rc<dyn Fn()>> {
        self.cancel.borrow().clone()
    }
    /// Replaces or removes cancellation delivery.
    pub fn set_on_cancel(&self, callback: Option<Rc<dyn Fn()>>) {
        let old = self.cancel.replace(callback);
        drop(old);
    }
    /// Publishes visible original indices after source-owned query filtering.
    fn filter_sessions(&self) {
        let sessions = self.sessions.borrow().clone();
        let query = self.input.get_value();
        let sort = self.sort_mode.get();
        let names = self.name_filter.get();
        let identities: std::collections::HashMap<_, _> = sessions
            .iter()
            .enumerate()
            .map(|(i, s)| (std::ptr::from_ref(s.as_ref()), i))
            .collect();
        let eligible: Vec<_> =
            filter_session_records(sessions.iter().map(AsRef::as_ref), &query, sort, names)
                .into_iter()
                .map(|s| identities[&std::ptr::from_ref(s)])
                .collect();
        let visible =
            if sort == SortMode::Threaded && query.trim_matches(is_whitespace_scalar).is_empty() {
                build_session_tree(&sessions, &eligible)
            } else {
                eligible
                    .into_iter()
                    .map(|index| FlatSessionNode {
                        index,
                        prefix: String::new(),
                    })
                    .collect()
            };
        self.selected
            .set(self.selected.get().min(visible.len().saturating_sub(1)));
        self.visible.replace(Rc::new(visible));
    }
    /// Snapshots handles and the live selected path before caller code runs.
    fn submit(&self) {
        if let (Some(path), Some(callback)) = (self.get_selected_session_path(), self.on_select()) {
            callback(&path);
        }
    }
    /// Samples the current foreground.
    fn paint(&self, key: &ThemeColor, text: &str) -> String {
        paint(self.options.theme.get().ok().as_deref(), key, text)
    }
    /// Composes count, displayed paths and live elapsed age.
    fn metadata(&self, session: &SessionInfo) -> String {
        let mut right = format!(
            "{} {}",
            session.message_count,
            format_session_date(session.modified, (self.options.now_ms)())
        );
        if self.show_cwd.get() && !session.cwd.is_empty() {
            right = format!("{} {right}", self.shortened(&session.cwd));
        }
        if self.show_path.get() {
            right = format!("{} {right}", self.shortened(&session.path));
        }
        right
    }
    /// Samples home at each displayed path, then removes terminal command controls.
    fn shortened(&self, path: &str) -> String {
        let home = (self.options.home)();
        display_text(&if path.is_empty() {
            String::new()
        } else {
            maestro_path::shorten_home(path, &home)
        })
    }
    /// Emits one record through the existing cell, style and clipping owners.
    fn row(&self, session: &SessionInfo, prefix: &str, selected: bool, width: usize) -> String {
        let confirming = self.confirming.borrow().as_deref() == Some(session.path.as_str());
        let active = self.is_current_path(&session.path);
        let message = display_text(session.name.as_deref().unwrap_or(&session.first_message));
        let right = self.metadata(session);
        let allowance = width
            .saturating_sub(2 + visible_width(prefix) + visible_width(&right) + 2)
            .max(10);
        let message = truncate_to_width(
            message.trim_matches(is_whitespace_scalar),
            allowance,
            TruncateOptions {
                ellipsis: "…",
                ..Default::default()
            },
        );
        let color = if confirming {
            Some(ThemeColor::Error)
        } else if active {
            Some(ThemeColor::Accent)
        } else if session.name.as_ref().is_some_and(|name| !name.is_empty()) {
            Some(ThemeColor::Warning)
        } else {
            None
        };
        let styled = match color {
            Some(color) => self.paint(&color, &message),
            None => message,
        };
        let styled = if selected {
            Theme::bold(&styled, (self.options.styles_enabled)())
        } else {
            styled
        };
        let cursor = if selected {
            self.paint(&ThemeColor::Accent, "› ")
        } else {
            "  ".into()
        };
        let left = cursor + &self.paint(&ThemeColor::Dim, prefix) + &styled;
        let spacing = width
            .saturating_sub(visible_width(&left) + visible_width(&right))
            .max(1);
        let line = left
            + &" ".repeat(spacing)
            + &self.paint(
                &if confirming {
                    ThemeColor::Error
                } else {
                    ThemeColor::Dim
                },
                &right,
            );
        let line = if selected {
            (super::super::background(self.options.theme.clone(), ThemeBg::SelectedBg))(&line)
        } else {
            line
        };
        truncate_to_width(&line, width, TruncateOptions::default())
    }
    /// Builds empty guidance only from currently enabled live actions.
    fn empty_message(&self) -> String {
        let named = self.name_filter.get() == NameFilter::Named;
        let cwd = self.show_cwd.get();
        let base = match (named, cwd) {
            (false, true) => "  No sessions found",
            (false, false) => "  No sessions in current folder.",
            (true, true) => "  No named sessions found.",
            (true, false) => "  No named sessions in current folder.",
        };
        let toggle = if named {
            maestro_app::key_text("app.session.toggleNamedFilter")
        } else {
            String::new()
        };
        let scope = if cwd {
            String::new()
        } else {
            maestro_app::key_text("tui.input.tab")
        };
        let scope = if scope == "tab" { "Tab".into() } else { scope };
        match (toggle.is_empty(), scope.is_empty()) {
            (false, false) => format!("{base} Press {toggle} to show all, or {scope} to view all."),
            (false, true) => format!("{base} Press {toggle} to show all."),
            (true, false) => format!("{base} Press {scope} to view all."),
            (true, true) => base.into(),
        }
    }
}
/// Replaces terminal command controls without changing surrounding whitespace.
fn display_text(text: &str) -> String {
    text.chars()
        .map(|c| {
            if matches!(c, '\0'..='\u{1f}' | '\u{7f}') {
                ' '
            } else {
                c
            }
        })
        .collect()
}
/// Formats fixed elapsed units rather than calendar dates.
fn format_session_date(modified: f64, now: f64) -> String {
    let minutes = ((now - modified) / 60000.0).floor();
    if minutes < 1.0 {
        return "now".into();
    }
    for (limit, divisor, suffix) in [
        (60.0, 1.0, "m"),
        (1440.0, 60.0, "h"),
        (10080.0, 1440.0, "d"),
        (43200.0, 10080.0, "w"),
        (525_600.0, 43200.0, "mo"),
        (f64::INFINITY, 525_600.0, "y"),
    ] {
        if minutes < limit {
            return format!("{}{suffix}", (minutes / divisor).floor());
        }
    }
    String::new()
}
impl Component for SessionList {
    fn render(&self, width: usize) -> Vec<String> {
        let sessions = self.sessions.borrow().clone();
        let visible = self.visible.borrow().clone();
        let mut lines = self.input.render(width);
        lines.push(String::new());
        if visible.is_empty() {
            lines.push(self.paint(
                &ThemeColor::Muted,
                &truncate_to_width(
                    &self.empty_message(),
                    width,
                    TruncateOptions {
                        ellipsis: "…",
                        ..Default::default()
                    },
                ),
            ));
            return lines;
        }
        let window = super::super::list_window(self.selected.get(), visible.len(), 10);
        for index in window.clone() {
            let node = &visible[index];
            lines.push(self.row(
                &sessions[node.index],
                &node.prefix,
                index == self.selected.get(),
                width,
            ));
        }
        if window.start > 0 || window.end < visible.len() {
            let position = format!("  ({}/{})", self.selected.get() + 1, visible.len());
            lines.push(self.paint(
                &ThemeColor::Muted,
                &truncate_to_width(
                    &position,
                    width,
                    TruncateOptions {
                        ellipsis: "",
                        ..Default::default()
                    },
                ),
            ));
        }
        lines
    }
    fn input_handler(&self) -> Option<&dyn InputHandler> {
        Some(self)
    }
    fn focusable(&self) -> Option<&dyn Focusable> {
        Some(self)
    }
}
impl Focusable for SessionList {
    fn focus_flag(&self) -> &FocusFlag {
        &self.focus
    }
}
impl InputHandler for SessionList {
    fn handle_input(&self, data: &str) {
        let bindings = maestro_tui::get_keybindings();
        if self.confirmation_input(data) || self.control_input(data) || self.mutation_input(data) {
            return;
        }
        let last = self.visible.borrow().len().saturating_sub(1);
        let selected = self.selected.get();
        if bindings.matches(data, "tui.select.up") {
            self.selected.set(selected.saturating_sub(1));
        } else if bindings.matches(data, "tui.select.down") {
            self.selected.set((selected + 1).min(last));
        } else if bindings.matches(data, "tui.select.pageUp") {
            self.selected.set(selected.saturating_sub(10));
        } else if bindings.matches(data, "tui.select.pageDown") {
            self.selected.set(selected.saturating_add(10).min(last));
        } else if bindings.matches(data, "tui.select.confirm") {
            self.submit();
        } else if bindings.matches(data, "tui.select.cancel") {
            if let Some(callback) = self.on_cancel() {
                callback();
            }
        } else {
            self.input.handle_input(data);
            self.filter_sessions();
        }
    }
}

#[cfg(test)]
pub(crate) mod tests;

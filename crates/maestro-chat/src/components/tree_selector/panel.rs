//! Bordered session-tree presentation over one retained navigation owner.
use super::{FilterMode, TreeList};
use crate::components::dynamic_border::DynamicBorder;
use maestro_app::key_text;
use maestro_theme::{LiveTheme, Theme, ThemeColor, paint};
use maestro_tui::tui::{FocusFlag, Focusable, InputHandler, TuiRuntime};
use maestro_tui::{
    Component, Container, Input, Spacer, Text, TruncateOptions, TruncatedText, truncate_to_width,
};
use std::{
    cell::RefCell,
    io,
    rc::{Rc, Weak},
    time::{Duration, SystemTime},
};

/// Selection and label notifications for a tree panel.
pub struct TreeSelectorCallbacks {
    /// Confirmed entry identity.
    pub on_select: Rc<dyn Fn(&str)>,
    /// Ordinary or initially empty-tree cancellation.
    pub on_cancel: Rc<dyn Fn()>,
    /// Saved label, with absence removing the label.
    pub on_label_change: Option<LabelChange>,
}
/// A saved-label notification.
type LabelChange = Rc<dyn Fn(&str, Option<&str>)>;
/// Supplied presentation and selection inputs.
pub struct TreeSelectorOptions {
    /// Active history leaf.
    pub current_leaf_id: Option<String>,
    /// Viewport height used to derive the page distance.
    pub terminal_height: usize,
    /// Initial selection overriding the leaf.
    pub initial_selected_id: Option<String>,
    /// Initial category selection.
    pub initial_filter_mode: Option<FilterMode>,
    /// Published colors for dynamic children.
    pub theme: LiveTheme,
    /// Host observations and deferred cancellation.
    pub runtime: Rc<dyn TuiRuntime>,
    /// Whether toolkit text decorations are enabled.
    pub styles_enabled: bool,
    /// Wall-clock observation shared by labels and timestamp rendering.
    pub now: Option<Rc<dyn Fn() -> SystemTime>>,
}
/// Render adapter retaining the same list as SDK callers.
struct ListView {
    /// Retained navigation owner.
    list: Rc<TreeList>,
    /// Live row theme.
    theme: LiveTheme,
    /// Host observations.
    runtime: Rc<dyn TuiRuntime>,
    /// Row decorations.
    styles: bool,
}
impl Component for ListView {
    fn render(&self, width: usize) -> Vec<String> {
        self.list
            .render(width, &self.theme, self.runtime.as_ref(), self.styles)
            .unwrap_or_default()
    }
    fn try_render(&self, width: usize) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        Ok(self
            .list
            .render(width, &self.theme, self.runtime.as_ref(), self.styles)?)
    }
}
/// Read-only authored-query display.
struct SearchLine {
    /// Existing query owner.
    list: Rc<TreeList>,
    /// Live search colors.
    theme: LiveTheme,
}
impl Component for SearchLine {
    fn render(&self, width: usize) -> Vec<String> {
        let theme = self.theme.get().ok();
        let mut line = format!(
            "  {}",
            paint(theme.as_deref(), &ThemeColor::Muted, "Type to search:")
        );
        let query = self.list.get_search_query();
        if !query.is_empty() {
            line.push(' ');
            line.push_str(&paint(theme.as_deref(), &ThemeColor::Accent, &query));
        }
        vec![truncate_to_width(&line, width, TruncateOptions::default())]
    }
}
/// A bordered session-tree panel retaining its list across child edits.
pub struct TreeSelectorComponent {
    /// Public child-array owner.
    container: Container,
    /// Retained list handle.
    list: Rc<TreeList>,
    /// Stable label editor.
    editor: Rc<LabelInput>,
    /// Shared focus forwarding to the editor.
    focus: FocusFlag,
}
impl TreeSelectorComponent {
    /// Builds the panel and schedules cancellation only for initially empty roots.
    ///
    /// # Errors
    /// Preserves reached list construction errors.
    pub fn new(
        tree: &[maestro_app::SessionTreeNode<'_>],
        callbacks: TreeSelectorCallbacks,
        options: TreeSelectorOptions,
    ) -> io::Result<Self> {
        let list = Rc::new(TreeList::new(
            tree,
            options.current_leaf_id.as_deref(),
            (options.terminal_height / 2).max(5),
            options.initial_selected_id.as_deref(),
            options.initial_filter_mode,
        )?);
        list.set_on_select(Some(callbacks.on_select));
        list.set_on_cancel(Some(callbacks.on_cancel.clone()));
        list.set_clock(options.now.unwrap_or_else(|| Rc::new(SystemTime::now)));
        let theme = options.theme;
        let tree_container = Rc::new(Container::new());
        let view = Rc::new(ListView {
            list: list.clone(),
            theme: theme.clone(),
            runtime: options.runtime.clone(),
            styles: options.styles_enabled,
        });
        tree_container.add_child(view.clone());
        let editor_container = Rc::new(Container::new());
        let editor = LabelInput::new(
            view,
            Rc::downgrade(&tree_container),
            Rc::downgrade(&editor_container),
            callbacks.on_label_change,
        );
        let container = chrome(
            theme,
            options.styles_enabled,
            list.clone(),
            tree_container,
            editor_container,
        );
        if tree.is_empty() {
            options.runtime.schedule(
                Duration::from_millis(100),
                Box::new(move || {
                    (callbacks.on_cancel)();
                    Ok(())
                }),
            );
        }
        Ok(Self {
            container,
            list,
            focus: FocusFlag::forwarding(editor.clone()),
            editor,
        })
    }
    /// Returns the retained navigation owner.
    #[must_use]
    pub fn get_tree_list(&self) -> &Rc<TreeList> {
        &self.list
    }
    /// Handles input with the list's native failure boundary.
    /// # Errors
    /// Preserves reached filtering errors.
    pub fn handle_input(&self, data: &str) -> io::Result<()> {
        let editing = self.editor.id.borrow().is_some();
        if editing {
            self.editor.handle_input(data);
            Ok(())
        } else {
            self.list.handle_input(data)
        }
    }
}
/// Composes live chrome around the retained list and editor containers.
fn chrome(
    theme: LiveTheme,
    styles: bool,
    list: Rc<TreeList>,
    tree: Rc<Container>,
    editor: Rc<Container>,
) -> Container {
    let container = Container::new();
    container.add_child(Rc::new(Spacer::new(1)));
    container.add_child(Rc::new(DynamicBorder::new(theme.clone(), None)));
    container.add_child(Rc::new(Text::new(
        Theme::bold("  Session Tree", styles),
        1,
        0,
        None,
    )));
    container.add_child(Rc::new(TruncatedText::new(
        paint(theme.get().ok().as_deref(), &ThemeColor::Muted, &help()),
        0,
        0,
    )));
    container.add_child(Rc::new(SearchLine {
        list,
        theme: theme.clone(),
    }));
    container.add_child(Rc::new(DynamicBorder::new(theme.clone(), None)));
    container.add_child(Rc::new(Spacer::new(1)));
    container.add_child(tree);
    container.add_child(editor);
    container.add_child(Rc::new(Spacer::new(1)));
    container.add_child(Rc::new(DynamicBorder::new(theme, None)));
    container
}
/// Samples constructor-only navigation hints from the active registry.
fn help() -> String {
    let filters = ["default", "noTools", "userOnly", "labeledOnly", "all"]
        .map(|name| key_text(&format!("app.tree.filter.{name}")))
        .join("/");
    format!(
        "  ↑/↓: move. ←/→: page. ^←/^→ or Alt+←/Alt+→: fold/branch. {}: label. {filters}: filters ({}/{} cycle). {}: label time",
        key_text("app.tree.editLabel"),
        key_text("app.tree.filter.cycleForward"),
        key_text("app.tree.filter.cycleBackward"),
        key_text("app.tree.toggleLabelTimestamp")
    )
}
component_children!(TreeSelectorComponent, Container, container);
impl Component for TreeSelectorComponent {
    fn render(&self, width: usize) -> Vec<String> {
        self.container.render(width)
    }
    fn try_render(&self, width: usize) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        self.container.try_render(width)
    }
    fn invalidate(&self) {
        self.container.invalidate();
    }
    fn input_handler(&self) -> Option<&dyn InputHandler> {
        Some(self)
    }
    fn focusable(&self) -> Option<&dyn Focusable> {
        Some(self)
    }
}
impl InputHandler for TreeSelectorComponent {
    fn handle_input(&self, data: &str) {
        let _ = Self::handle_input(self, data);
    }
}

impl Focusable for TreeSelectorComponent {
    fn focus_flag(&self) -> &FocusFlag {
        &self.focus
    }
}

/// Stable editing widget with non-owning container backedges.
struct LabelInput {
    /// Freshly reset editing owner.
    input: Rc<Input>,
    /// Entry identity captured at opening.
    id: RefCell<Option<String>>,
    /// List container restored on closure.
    tree: Weak<Container>,
    /// Container containing this editor only while open.
    container: Weak<Container>,
    /// Retained list presentation.
    view: Rc<ListView>,
    /// Published editor colors.
    theme: LiveTheme,
    /// Saved-label notification.
    callback: Option<LabelChange>,
    /// Existing forwarding owner.
    focus: FocusFlag,
}
impl LabelInput {
    /// Creates one editor and installs its non-owning notification.
    fn new(
        view: Rc<ListView>,
        tree: Weak<Container>,
        container: Weak<Container>,
        callback: Option<LabelChange>,
    ) -> Rc<Self> {
        let input = Rc::new(Input::new());
        let editor = Rc::new(Self {
            focus: FocusFlag::forwarding(input.clone()),
            input,
            id: RefCell::new(None),
            tree,
            container,
            theme: view.theme.clone(),
            view,
            callback,
        });
        let weak = Rc::downgrade(&editor);
        editor
            .view
            .list
            .set_on_label_edit(Some(Rc::new(move |id, label| {
                if let Some(editor) = weak.upgrade() {
                    editor.open(id, label.as_ref().and_then(serde_json::Value::as_str));
                }
            })));
        editor
    }
    /// Opens the same widget with fresh editing state.
    fn open(self: &Rc<Self>, id: &str, value: Option<&str>) {
        self.input.reset(value.unwrap_or_default().to_owned());
        *self.id.borrow_mut() = Some(id.to_owned());
        if let (Some(tree), Some(container)) = (self.tree.upgrade(), self.container.upgrade()) {
            tree.clear();
            container.clear();
            container.add_child(self.clone());
        }
    }
    /// Restores the same navigation presentation after normal return.
    fn hide(&self) {
        *self.id.borrow_mut() = None;
        if let (Some(tree), Some(container)) = (self.tree.upgrade(), self.container.upgrade()) {
            container.clear();
            tree.clear();
            tree.add_child(self.view.clone());
        }
    }
}
impl Component for LabelInput {
    fn render(&self, width: usize) -> Vec<String> {
        let theme = self.theme.get().ok();
        let mut lines = vec![format!(
            "  {}",
            paint(
                theme.as_deref(),
                &ThemeColor::Muted,
                "Label (empty to remove):"
            )
        )];
        lines.extend(
            self.input
                .render(width.saturating_sub(2))
                .iter()
                .map(|line| format!("  {line}")),
        );
        let hint = maestro_app::key_hint("tui.select.confirm", "save", &self.theme)
            .unwrap_or_default()
            + "  "
            + &maestro_app::key_hint("tui.select.cancel", "cancel", &self.theme)
                .unwrap_or_default();
        lines.push(format!("  {hint}"));
        lines
            .iter()
            .map(|line| truncate_to_width(line, width, TruncateOptions::default()))
            .collect()
    }
}
impl Focusable for LabelInput {
    fn focus_flag(&self) -> &FocusFlag {
        &self.focus
    }
}
impl InputHandler for LabelInput {
    fn handle_input(&self, data: &str) {
        let keys = maestro_tui::get_keybindings();
        if keys.matches(data, "tui.select.confirm") {
            let value = self.input.get_value();
            let value = value.trim_matches(maestro_app::is_whitespace_scalar);
            let label = (!value.is_empty()).then_some(value);
            let id = self.id.borrow().clone();
            let Some(id) = id else {
                return;
            };
            self.view.list.update_node_label(&id, label, None);
            if let Some(callback) = &self.callback {
                callback(&id, label);
            }
            self.hide();
        } else if keys.matches(data, "tui.select.cancel") {
            self.hide();
        } else {
            self.input.handle_input(data);
        }
    }
}

#[cfg(test)]
mod tests;

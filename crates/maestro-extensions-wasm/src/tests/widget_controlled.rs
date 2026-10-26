//! The controlled host's widgets: the shared fake, with nested calls made directly.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use serde_json::json;

use super::controlled::Controlled;
use super::widget_host::{self, Build, CODE, Change, Child, Paint, Part, ROLES};
use super::widget_lists::{self, Call, Event, List, Setting};
use crate::component_adapter::renderers::with_emitter;
use crate::component_adapter::{
    ChildKey, ChildRef, HandlerSlot, HighlightRef, HostMarkdownTheme, HostSettingsTheme, Imports,
    LayoutRef, MarkdownStyles, RowStyleRef, SettingRef, SettingsStyles, StyleRef, TextStyle,
    WidgetAnswer, WidgetEvent, WidgetImports, authored_component, invoke_highlight, invoke_style,
    invoke_widget,
};
use crate::types::ExtensionResult;

/// A host widget by key.
pub struct Widget(pub u32);

/// A host style by role.
pub struct Style(pub String);

/// The host's code highlighting, by name.
pub struct Highlighter(pub String);

impl Controlled {
    /// The paint a styling operand names.
    fn paint(&self, style: &StyleRef<'_, Self>) -> Paint {
        match style {
            StyleRef::Host(style) => Paint::Host(style.0.clone()),
            StyleRef::Authored(callback) => Paint::Authored(self.callback_id(callback)),
        }
    }

    /// The child an operand names.
    fn child(&self, child: &ChildRef<'_, Self>) -> Child {
        match child {
            ChildRef::Widget(widget) => Child::Widget(widget.0),
            ChildRef::Component(callback) => Child::Component(self.callback_id(callback)),
        }
    }

    /// Records and builds one widget.
    fn build_widget(&self, build: Build) -> Widget {
        Widget(widget_host::build(&mut self.observe(), build))
    }

    /// Records and applies one change.
    fn change(&self, widget: &Widget, change: Change) -> ExtensionResult<()> {
        widget_host::change(&mut self.observe(), widget.0, change)
    }

    /// Runs one authored component method with a lent emitter.
    fn component<R>(
        &self,
        id: u32,
        method: impl FnOnce(&crate::Component, crate::CallbackEmitter<'_>) -> ExtensionResult<R>,
    ) -> ExtensionResult<R> {
        let component = match self.kept_component(id) {
            Some(component) => component,
            None => authored_component(id)?,
        };
        let emitter = super::bus_host::Emitter { key: 0 };
        with_emitter(self, &emitter, |emitter| method(&component, emitter))
    }
}

impl WidgetImports for Controlled {
    type Widget = Widget;
    type Style = Style;
    type Highlighter = Highlighter;
    type RowStyle = RowStyle;
    type SubmenuDone = SubmenuDone;

    fn new_select_list(
        &self,
        items: &[crate::SelectItem],
        max_visible: u32,
        theme: Vec<StyleRef<'_, Self>>,
        (min, max, truncate): LayoutRef<'_, Self>,
    ) -> ExtensionResult<Widget> {
        let styles: Vec<_> = theme.iter().map(|style| self.paint(style)).collect();
        let truncate = truncate.map(|callback| self.callback_id(callback));
        let operands = json!({ "items": items.iter().map(|item| [&item.value, &item.label])
            .collect::<Vec<_>>(), "maxVisible": max_visible,
            "styles": styles.iter().map(|style| format!("{style:?}")).collect::<Vec<_>>(),
            "min": min, "max": max, "truncate": truncate.is_some() });
        let list = List {
            items: items.to_vec(),
            selected_style: styles.get(1).cloned(),
            truncate,
            ..List::default()
        };
        Ok(self.build_list("select", operands, list))
    }
    fn new_settings_list(
        &self,
        (items, max_visible): (&[SettingRef<'_, Self>], u32),
        theme: SettingsStyles<'_, Self>,
        (on_change, on_cancel): (&super::controlled::Callback, &super::controlled::Callback),
        enable_search: bool,
    ) -> ExtensionResult<Widget> {
        let settings: Vec<_> = items
            .iter()
            .map(|setting| Setting {
                id: setting.item.id.clone(),
                label: setting.item.label.clone(),
                value: setting.item.current_value.clone(),
                values: setting.item.values.clone(),
                submenu: setting.submenu.map(|callback| self.callback_id(callback)),
            })
            .collect();
        let rows = (self.row_paint(&theme.label), self.row_paint(&theme.value));
        let operands = json!({ "ids": settings.iter().map(|setting| &setting.id).collect::<Vec<_>>(),
            "maxVisible": max_visible, "rows": format!("{rows:?}"), "cursor": theme.cursor,
            "description": format!("{:?}", self.paint(&theme.description)),
            "hint": format!("{:?}", self.paint(&theme.hint)), "search": enable_search });
        let mut list = List {
            settings,
            rows: Some(rows),
            ..List::default()
        };
        list.handlers.insert("change", self.callback_id(on_change));
        list.handlers.insert("cancel", self.callback_id(on_cancel));
        Ok(self.build_list("settings", operands, list))
    }
    fn new_bordered_loader(
        &self,
        _: &super::controlled::Tui,
        theme: &super::controlled::Theme,
        message: &str,
        cancellable: Option<bool>,
    ) -> ExtensionResult<Widget> {
        let operands =
            json!({ "theme": theme.key, "message": message, "cancellable": cancellable });
        let list = List {
            loader: Some((message.to_owned(), cancellable.unwrap_or(true))),
            ..List::default()
        };
        Ok(self.build_list("loader", operands, list))
    }
    fn get_select_list_theme(&self) -> ExtensionResult<Vec<Style>> {
        self.observe().log("getSelectListTheme");
        let roles = [
            "selectedPrefix",
            "selectedText",
            "description",
            "scrollInfo",
            "noMatch",
        ];
        Ok(roles
            .into_iter()
            .map(|role| Style(role.to_owned()))
            .collect())
    }
    fn get_settings_list_theme(&self) -> ExtensionResult<HostSettingsTheme<Self>> {
        self.observe().log("getSettingsListTheme");
        Ok((
            RowStyle("label".to_owned()),
            RowStyle("value".to_owned()),
            Style("description".to_owned()),
            "→ ".to_owned(),
            Style("hint".to_owned()),
        ))
    }
    fn widget_handle_input(&self, widget: &Widget, data: &str) -> ExtensionResult<()> {
        let calls = widget_lists::input(&mut self.observe(), widget.0, data)?;
        self.calls(widget.0, calls)
    }
    fn widget_set_filter(&self, widget: &Widget, filter: &str) -> ExtensionResult<()> {
        let mut observed = self.observe();
        observed.log(format!("widget {} setFilter {}", widget.0, json!(filter)));
        let list = widget_lists::list(&mut observed, widget.0)?;
        let filter = filter.to_lowercase();
        list.filtered = (0..list.items.len())
            .filter(|&index| list.items[index].value.to_lowercase().starts_with(&filter))
            .collect();
        list.selected = 0;
        Ok(())
    }
    fn widget_set_selected_index(&self, widget: &Widget, index: i64) -> ExtensionResult<()> {
        let mut observed = self.observe();
        observed.log(format!("widget {} setSelectedIndex {index}", widget.0));
        let list = widget_lists::list(&mut observed, widget.0)?;
        let final_row = i64::try_from(list.filtered.len()).map_err(|error| error.to_string())? - 1;
        let index =
            usize::try_from(index.min(final_row).max(0)).map_err(|error| error.to_string())?;
        list.selected = index;
        Ok(())
    }
    fn widget_get_selected_item(
        &self,
        widget: &Widget,
    ) -> ExtensionResult<Option<crate::SelectItem>> {
        let mut observed = self.observe();
        Ok(widget_lists::selected(widget_lists::list(
            &mut observed,
            widget.0,
        )?))
    }
    fn widget_set_handler(
        &self,
        widget: &Widget,
        slot: HandlerSlot,
        handler: Option<&super::controlled::Callback>,
    ) -> ExtensionResult<()> {
        let slot = match slot {
            HandlerSlot::Select => "select",
            HandlerSlot::Cancel => "cancel",
            HandlerSlot::SelectionChange => "selectionChange",
            HandlerSlot::Abort => "abort",
        };
        let handler = handler.map(|handler| self.callback_id(handler));
        let mut observed = self.observe();
        observed.log(format!(
            "widget {} setHandler {slot} {}",
            widget.0,
            handler.is_some()
        ));
        let list = widget_lists::list(&mut observed, widget.0)?;
        match handler {
            Some(handler) => list.handlers.insert(slot, handler),
            None => list.handlers.remove(slot),
        };
        Ok(())
    }
    fn widget_update_value(&self, widget: &Widget, id: &str, value: &str) -> ExtensionResult<()> {
        let mut observed = self.observe();
        observed.log(format!(
            "widget {} updateValue {}",
            widget.0,
            json!([id, value])
        ));
        let list = widget_lists::list(&mut observed, widget.0)?;
        for setting in list.settings.iter_mut().filter(|setting| setting.id == id) {
            setting.value = value.to_owned();
        }
        Ok(())
    }
    fn widget_signal(&self, widget: &Widget) -> ExtensionResult<super::controlled::Signal> {
        let flag = widget_lists::list(&mut self.observe(), widget.0)?
            .signal
            .clone();
        Ok(self.signal(&super::controlled::Flag(flag)))
    }
    fn widget_dispose(&self, widget: &Widget) -> ExtensionResult<()> {
        self.observe().log(format!("widget {} dispose", widget.0));
        Ok(())
    }
    fn row_style_apply(
        &self,
        style: &RowStyle,
        text: &str,
        selected: bool,
    ) -> ExtensionResult<String> {
        self.observe()
            .log(format!("rowStyle {} {}", style.0, json!([text, selected])));
        Ok(format!("<{} {selected}>{text}", style.0))
    }
    fn submenu_done(&self, done: &SubmenuDone, value: Option<&str>) -> ExtensionResult<()> {
        let change = widget_lists::done(&mut self.observe(), done.0, value)?;
        self.calls(done.0, change.into_iter().collect())
    }

    fn new_text(
        &self,
        text: &str,
        padding: (u32, u32),
        background: Option<StyleRef<'_, Self>>,
    ) -> ExtensionResult<Widget> {
        let paint = background.map(|style| self.paint(&style));
        Ok(self.build_widget(Build {
            kind: "text",
            text: text.to_owned(),
            padding,
            paint,
            ..Build::default()
        }))
    }
    fn new_box(
        &self,
        padding: (u32, u32),
        background: Option<StyleRef<'_, Self>>,
    ) -> ExtensionResult<Widget> {
        let paint = background.map(|style| self.paint(&style));
        Ok(self.build_widget(Build {
            kind: "box",
            padding,
            paint,
            ..Build::default()
        }))
    }
    fn new_container(&self) -> ExtensionResult<Widget> {
        Ok(self.build_widget(Build {
            kind: "container",
            ..Build::default()
        }))
    }
    fn new_spacer(&self, lines: u32) -> ExtensionResult<Widget> {
        Ok(self.build_widget(Build {
            kind: "spacer",
            lines,
            ..Build::default()
        }))
    }
    fn new_markdown(
        &self,
        text: &str,
        padding: (u32, u32),
        theme: MarkdownStyles<'_, Self>,
        style: Option<TextStyle<'_, Self>>,
    ) -> ExtensionResult<Widget> {
        let roles: Vec<_> = theme.roles.iter().map(|role| self.paint(role)).collect();
        let highlight = theme.highlight.map(|highlight| match highlight {
            HighlightRef::Host(highlighter) => Paint::Host(highlighter.0.clone()),
            HighlightRef::Authored(callback) => Paint::Authored(self.callback_id(callback)),
        });
        let style = style.map(|style| {
            let color = style.color.map(|color| format!("{:?}", self.paint(&color)));
            let bg = style.bg_color.map(|bg| format!("{:?}", self.paint(&bg)));
            let decorations: Vec<_> = style
                .decorations
                .iter()
                .map(|decoration| format!("{decoration:?}"))
                .collect();
            json!([color, bg, decorations])
        });
        let operands = json!({
            "roles": roles.iter().map(|role| format!("{role:?}")).collect::<Vec<_>>(),
            "highlight": highlight.as_ref().map(|paint| format!("{paint:?}")),
            "indent": theme.indent,
            "style": style,
        });
        Ok(self.build_widget(Build {
            kind: "markdown",
            text: text.to_owned(),
            padding,
            paint: roles.into_iter().next(),
            highlight,
            operands,
            ..Build::default()
        }))
    }
    fn new_dynamic_border(&self, color: Option<StyleRef<'_, Self>>) -> ExtensionResult<Widget> {
        let paint = color.map(|style| self.paint(&style));
        Ok(self.build_widget(Build {
            kind: "border",
            paint,
            ..Build::default()
        }))
    }
    fn get_markdown_theme(&self) -> ExtensionResult<HostMarkdownTheme<Self>> {
        self.observe().log("getMarkdownTheme");
        let styles = ROLES.iter().map(|role| Style((*role).to_owned())).collect();
        Ok((styles, Highlighter("hl".to_owned())))
    }
    fn widget_render(&self, widget: &Widget, width: u32) -> ExtensionResult<Vec<String>> {
        let parts = widget_host::render(&mut self.observe(), widget.0, width)?;
        let mut rows = Vec::new();
        for part in parts {
            match part {
                Part::Row(row) => rows.push(row),
                Part::Style(id, row) => rows.push(invoke_style(id, &row)?),
                Part::Component(id) => {
                    rows.extend(self.component(id, |view, emitter| view.render(width, emitter))?);
                }
                Part::Highlight(id) => rows.extend(invoke_highlight(id, CODE.0, Some(CODE.1))?),
                Part::Widget(id, event) => rows.push(self.widget_text(id, widget.0, event)?),
            }
        }
        Ok(rows)
    }
    fn widget_invalidate(&self, widget: &Widget) -> ExtensionResult<()> {
        let components = widget_host::invalidate(&mut self.observe(), widget.0)?;
        for id in components {
            self.component(id, crate::Component::invalidate)?;
        }
        Ok(())
    }
    fn widget_set_text(&self, widget: &Widget, text: &str) -> ExtensionResult<()> {
        self.change(widget, Change::Text(text.to_owned()))
    }
    fn widget_set_background(
        &self,
        widget: &Widget,
        style: Option<StyleRef<'_, Self>>,
    ) -> ExtensionResult<()> {
        let paint = style.map(|style| self.paint(&style));
        self.change(widget, Change::Background(paint))
    }
    fn widget_set_lines(&self, widget: &Widget, lines: u32) -> ExtensionResult<()> {
        self.change(widget, Change::Lines(lines))
    }
    fn widget_add_child(&self, widget: &Widget, child: ChildRef<'_, Self>) -> ExtensionResult<()> {
        let child = self.child(&child);
        self.change(widget, Change::Add(child))
    }
    fn widget_remove_child(
        &self,
        widget: &Widget,
        child: ChildKey<'_, Self>,
    ) -> ExtensionResult<()> {
        let child = match child {
            ChildKey::Widget(child) => Child::Widget(child.0),
            ChildKey::Component(id) => Child::Component(id),
        };
        self.change(widget, Change::Remove(child))
    }
    fn widget_clear(&self, widget: &Widget) -> ExtensionResult<()> {
        self.change(widget, Change::Clear)
    }
    fn style_apply(&self, style: &Style, text: &str) -> ExtensionResult<String> {
        Ok(widget_host::apply(&mut self.observe(), &style.0, text))
    }
    fn highlight(
        &self,
        _: &Highlighter,
        code: &str,
        lang: Option<&str>,
    ) -> ExtensionResult<Vec<String>> {
        Ok(widget_host::highlight(&mut self.observe(), code, lang))
    }
}

/// A host settings row style by role.
pub struct RowStyle(pub String);

/// Closes the open submenu of the settings list under this key.
pub struct SubmenuDone(pub u32);

impl Controlled {
    /// The paint a row styling operand names.
    fn row_paint(&self, style: &RowStyleRef<'_, Self>) -> Paint {
        match style {
            RowStyleRef::Host(style) => Paint::Host(style.0.clone()),
            RowStyleRef::Authored(callback) => Paint::Authored(self.callback_id(callback)),
        }
    }

    /// Runs one authored widget callback with a lent emitter.
    fn widget_call(&self, id: u32, event: WidgetEvent<Self>) -> ExtensionResult<WidgetAnswer> {
        let emitter = super::bus_host::Emitter { key: 0 };
        invoke_widget(self, id, event, &emitter)
    }

    /// The event the host tells an authored callback for the list under `key`.
    fn widget_event(key: u32, event: Event) -> WidgetEvent<Self> {
        match event {
            Event::Select(item) => WidgetEvent::Select(item),
            Event::Cancel => WidgetEvent::Cancel,
            Event::SelectionChange(item) => WidgetEvent::SelectionChange(item),
            Event::Change(id, value) => WidgetEvent::Change(id, value),
            Event::Abort => WidgetEvent::Abort,
            Event::Truncate(item, width, is_selected) => {
                WidgetEvent::Truncate(crate::SelectListTruncatePrimaryContext {
                    text: item.label.clone(),
                    max_width: width - 2,
                    column_width: 32,
                    item,
                    is_selected,
                })
            }
            Event::Row(text, selected) => WidgetEvent::Row(text, selected),
            Event::Submenu(_, value) => WidgetEvent::Submenu(value, SubmenuDone(key)),
        }
    }

    /// Makes each nested call an input of the list under `key` makes.
    fn calls(&self, key: u32, calls: Vec<Call>) -> ExtensionResult<()> {
        for call in calls {
            match call {
                Call::Widget(id, event) => {
                    let index = match &event {
                        Event::Submenu(index, _) => Some(*index),
                        _ => None,
                    };
                    let answer = self.widget_call(id, Self::widget_event(key, event))?;
                    self.open_submenu(key, index, answer)?;
                }
                Call::Input(id, data) => {
                    self.component(id, |view, emitter| view.handle_input(&data, emitter))?;
                }
            }
        }
        Ok(())
    }

    /// Opens the submenu a setting's callback answered with.
    fn open_submenu(
        &self,
        key: u32,
        index: Option<usize>,
        answer: WidgetAnswer,
    ) -> ExtensionResult<()> {
        if let (Some(index), WidgetAnswer::Component(component)) = (index, answer) {
            let handle = self.keep_component(component);
            widget_lists::list(&mut self.observe(), key)?.submenu = Some((index, handle));
        }
        Ok(())
    }

    /// The text an authored widget callback answers.
    fn widget_text(&self, id: u32, key: u32, event: Event) -> ExtensionResult<String> {
        match self.widget_call(id, Self::widget_event(key, event))? {
            WidgetAnswer::Text(text) => Ok(text),
            _ => Err("expected text".to_owned()),
        }
    }

    /// Records a list widget and keeps its state.
    fn build_list(&self, kind: &'static str, operands: serde_json::Value, list: List) -> Widget {
        let widget = self.build_widget(Build {
            kind,
            operands,
            ..Build::default()
        });
        widget_lists::keep(&mut self.observe(), widget.0, list);
        widget
    }
}

//! The controlled host's widgets: the shared fake, with nested calls made directly.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use serde_json::json;

use super::controlled::Controlled;
use super::provider_host;
use super::widget_editors;
use super::widget_host::{self, Build, CODE, Change, Child, Paint, Part, ROLES};
use super::widget_keys::{self, Answer as KeysAnswer, Request as KeysRequest};
use super::widget_lists::{self, Call, Event, List, Setting};
use crate::component_adapter::renderers::with_emitter;
use crate::component_adapter::{
    ChildKey, ChildRef, EditorAnswer, EditorRequest, EditorStyles, HandlerSlot, HighlightRef,
    HostMarkdownTheme, HostSettingsTheme, Imports, KeyHint, KeybindingsAnswer, KeybindingsRequest,
    LayoutRef, MarkdownStyles, RowStyleRef, SettingRef, SettingsStyles, StyleRef, TextStyle,
    WidgetAnswer, WidgetEvent, WidgetImports, authored_component, invoke_highlight, invoke_style,
    invoke_widget,
};
use crate::types::ExtensionResult;

/// A host widget by key.
pub struct Widget(pub u32);

/// A host style by role.
pub struct Style(pub String);

/// A text callback lent to an authored editor, by name.
pub struct TextCallback(pub String);

/// An editor factory another extension set, with whether it builds a custom editor.
pub struct ForeignFactory(pub bool);

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

/// The answer of an input listener request; listeners are exercised on the native host.
const NO_WRITER: &str = "the controlled host keeps no input listeners";

impl WidgetImports for Controlled {
    fn global_theme(&self) -> ExtensionResult<super::controlled::Theme> {
        Ok(self.theme("dark"))
    }
    type Widget = Widget;
    type Style = Style;
    type Highlighter = Highlighter;
    type Keybindings = Keybindings;

    fn new_keybindings(
        &self,
        user_bindings: &crate::KeybindingsConfig,
        config_path: Option<&str>,
    ) -> ExtensionResult<Keybindings> {
        let path = config_path.map(str::to_owned);
        Ok(Keybindings(widget_keys::keep(
            &mut self.observe(),
            None,
            user_bindings.clone(),
            path,
        )))
    }
    fn create_keybindings(&self, agent_dir: Option<&str>) -> ExtensionResult<Keybindings> {
        let path = format!("{}/keybindings.json", agent_dir.unwrap_or("/agent"));
        Ok(Keybindings(widget_keys::keep(
            &mut self.observe(),
            None,
            Vec::new(),
            Some(path),
        )))
    }
    fn keybindings_with_definitions(
        &self,
        definitions: &crate::KeybindingDefinitions,
        user_bindings: &crate::KeybindingsConfig,
    ) -> ExtensionResult<Keybindings> {
        let definitions = Some(definitions.clone());
        let key = widget_keys::keep(
            &mut self.observe(),
            definitions,
            user_bindings.clone(),
            None,
        );
        Ok(Keybindings(key))
    }
    fn get_keybindings(&self) -> ExtensionResult<Keybindings> {
        self.observe().log("getKeybindings");
        Ok(Keybindings(0))
    }
    fn keybindings_request(
        &self,
        manager: &Keybindings,
        request: KeybindingsRequest<'_>,
    ) -> ExtensionResult<KeybindingsAnswer> {
        let answer = widget_keys::request(&mut self.observe(), manager.0, keys_request(request));
        Ok(keys_answer(answer))
    }
    fn key_hint(&self, hint: KeyHint<'_>) -> ExtensionResult<String> {
        let mut observed = self.observe();
        Ok(match hint {
            KeyHint::Text(id) => widget_keys::hint(&mut observed, Some(id), None, None),
            KeyHint::Keybinding(id, text) => {
                widget_keys::hint(&mut observed, Some(id), None, Some(text))
            }
            KeyHint::Raw(key, text) => {
                widget_keys::hint(&mut observed, None, Some(key), Some(text))
            }
        })
    }
    fn get_editor_theme(&self) -> ExtensionResult<(Style, Vec<Style>)> {
        self.observe().log("getEditorTheme");
        let roles = [
            "selectedPrefix",
            "selectedText",
            "description",
            "scrollInfo",
            "noMatch",
        ];
        let styles = roles
            .into_iter()
            .map(|role| Style(role.to_owned()))
            .collect();
        Ok((Style("borderMuted".to_owned()), styles))
    }
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
        let mut observed = self.observe();
        let calls = if widget_editors::is_editor(&observed, widget.0) {
            widget_editors::input(&mut observed, widget.0, data)
        } else {
            widget_lists::input(&mut observed, widget.0, data)
        };
        drop(observed);
        self.calls(widget.0, calls?)
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
        let slot = super::widget_editors::slot_name(slot);
        let handler = handler.map(|handler| self.callback_id(handler));
        let mut observed = self.observe();
        observed.log(format!(
            "widget {} setHandler {slot} {}",
            widget.0,
            handler.is_some()
        ));
        let handlers = if widget_editors::is_editor(&observed, widget.0) {
            &mut widget_editors::editor(&mut observed, widget.0)?.handlers
        } else {
            &mut widget_lists::list(&mut observed, widget.0)?.handlers
        };
        match handler {
            Some(handler) => handlers.insert(slot, handler),
            None => handlers.remove(slot),
        };
        Ok(())
    }
    fn new_editor(
        &self,
        _: &super::controlled::Tui,
        theme: EditorStyles<'_, Self>,
        options: crate::EditorOptions,
    ) -> ExtensionResult<Widget> {
        Ok(self.build_editor(&theme, None, options))
    }
    fn new_custom_editor(
        &self,
        _: &super::controlled::Tui,
        theme: EditorStyles<'_, Self>,
        keybindings: &Keybindings,
        options: crate::EditorOptions,
    ) -> ExtensionResult<Widget> {
        Ok(self.build_editor(&theme, Some(keybindings.0), options))
    }
    fn widget_editor(
        &self,
        widget: &Widget,
        request: EditorRequest<'_, Self>,
    ) -> ExtensionResult<EditorAnswer> {
        match self.editor_request(request) {
            widget_editors::Request::Edit(text, insert) => {
                let calls = widget_editors::edit(&mut self.observe(), widget.0, &text, insert)?;
                self.calls(widget.0, calls).map(|()| EditorAnswer::Done)
            }
            request => {
                let answer = widget_editors::request(&mut self.observe(), widget.0, request)?;
                Ok(editor_answer(answer))
            }
        }
    }
    fn widget_on_action(
        &self,
        widget: &Widget,
        action: &str,
        handler: &super::controlled::Callback,
    ) -> ExtensionResult<()> {
        let handler = self.callback_id(handler);
        widget_editors::on_action(&mut self.observe(), widget.0, action, handler)
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
    fn tui_writer(&self, _: &super::controlled::Tui) -> ExtensionResult<Widget> {
        self.new_container()
    }
    fn widget_writer(
        &self,
        widget: &Widget,
        request: crate::component_adapter::WriterRequest<'_, Self>,
    ) -> ExtensionResult<crate::component_adapter::WriterAnswer<Self>> {
        Ok(self.writer_request(widget, &request))
    }
    fn widget_add_input_listener(
        &self,
        widget: &Widget,
        listener: &super::controlled::Callback,
    ) -> ExtensionResult<super::bus_host::Subscription> {
        let line = format!("writer {} addInputListener {}", widget.0, listener.id);
        self.observe().log(line);
        Err(NO_WRITER.to_owned())
    }
    fn widget_drain_input<'a>(
        &'a self,
        _: &'a Widget,
        max_ms: Option<f64>,
        idle_ms: Option<f64>,
    ) -> crate::ExtensionFuture<'a, ()> {
        let line = format!("writer drainInput {max_ms:?} {idle_ms:?}");
        self.observe().log(line);
        Box::pin(std::future::ready(Ok(())))
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
            Event::Text(text) => WidgetEvent::Text(text),
            Event::Notice => WidgetEvent::Notice,
            Event::Shortcut(data) => WidgetEvent::Shortcut(data),
        }
    }

    /// Makes each nested call an input of the list under `key` makes.
    fn calls(&self, key: u32, calls: Vec<Call>) -> ExtensionResult<()> {
        let mut calls = std::collections::VecDeque::from(calls);
        while let Some(call) = calls.pop_front() {
            match call {
                Call::Widget(id, Event::Shortcut(data)) => {
                    calls.extend(self.shortcut(key, id, &data)?);
                }
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

    /// Offers input to an editor's interceptor and returns the calls the input makes after it.
    fn shortcut(&self, key: u32, id: u32, data: &str) -> ExtensionResult<Vec<Call>> {
        let event = Self::widget_event(key, Event::Shortcut(data.to_owned()));
        match self.widget_call(id, event)? {
            WidgetAnswer::Consumed(false) => {
                widget_editors::proceed(&mut self.observe(), key, data)
            }
            _ => Ok(Vec::new()),
        }
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

/// A host keybindings manager by key.
pub struct Keybindings(pub u32);

/// The owned form of a keybindings request.
pub fn keys_request(request: KeybindingsRequest<'_>) -> KeysRequest {
    match request {
        KeybindingsRequest::Matches(data, id) => {
            KeysRequest::Matches(data.to_owned(), id.to_owned())
        }
        KeybindingsRequest::Keys(id) => KeysRequest::Keys(id.to_owned()),
        KeybindingsRequest::Definition(id) => KeysRequest::Definition(id.to_owned()),
        KeybindingsRequest::Conflicts => KeysRequest::Conflicts,
        KeybindingsRequest::SetUserBindings(config) => KeysRequest::SetUserBindings(config.clone()),
        KeybindingsRequest::UserBindings => KeysRequest::UserBindings,
        KeybindingsRequest::ResolvedBindings => KeysRequest::ResolvedBindings,
        KeybindingsRequest::Reload => KeysRequest::Reload,
        KeybindingsRequest::EffectiveConfig => KeysRequest::EffectiveConfig,
    }
}

/// The adapter form of a keybindings answer.
fn keys_answer(answer: KeysAnswer) -> KeybindingsAnswer {
    match answer {
        KeysAnswer::Done => KeybindingsAnswer::Done,
        KeysAnswer::Matched(matched) => KeybindingsAnswer::Matched(matched),
        KeysAnswer::Keys(keys) => KeybindingsAnswer::Keys(keys),
        KeysAnswer::Definition(definition) => KeybindingsAnswer::Definition(definition),
        KeysAnswer::Conflicts(conflicts) => KeybindingsAnswer::Conflicts(conflicts),
        KeysAnswer::Config(config) => KeybindingsAnswer::Config(config),
    }
}

impl Controlled {
    /// Records an editor widget and keeps its state.
    fn build_editor(
        &self,
        theme: &EditorStyles<'_, Self>,
        keys: Option<u32>,
        options: crate::EditorOptions,
    ) -> Widget {
        let border = self.paint(&theme.border);
        let list: Vec<_> = theme
            .select_list
            .iter()
            .map(|style| self.paint(style))
            .collect();
        let options = (options.padding_x, options.autocomplete_max_visible);
        let operands = widget_editors::operands(&border, &list, keys, options);
        let kind = if keys.is_some() {
            "customEditor"
        } else {
            "editor"
        };
        let widget = self.build_widget(Build {
            kind,
            operands,
            ..Build::default()
        });
        let mut editor = widget_editors::Editor::new(border, options);
        editor.keys = keys;
        widget_editors::keep(&mut self.observe(), widget.0, editor);
        widget
    }

    /// The owned form of an editor request that makes no nested call.
    fn editor_request(&self, request: EditorRequest<'_, Self>) -> widget_editors::Request {
        use widget_editors::Request;
        match request {
            EditorRequest::SetBorderColor(color) => Request::SetBorderColor(self.paint(&color)),
            EditorRequest::SetText(text) => Request::Edit(text.to_owned(), false),
            EditorRequest::InsertTextAtCursor(text) => Request::Edit(text.to_owned(), true),
            EditorRequest::GetText => Request::GetText,
            EditorRequest::GetExpandedText => Request::GetExpandedText,
            EditorRequest::GetLines => Request::GetLines,
            EditorRequest::GetCursor => Request::GetCursor,
            EditorRequest::AddToHistory(text) => Request::AddToHistory(text.to_owned()),
            EditorRequest::GetPaddingX => Request::GetPaddingX,
            EditorRequest::SetPaddingX(padding) => Request::SetPaddingX(padding),
            EditorRequest::GetAutocompleteMaxVisible => Request::GetAutocompleteMaxVisible,
            EditorRequest::SetAutocompleteMaxVisible(max) => {
                Request::SetAutocompleteMaxVisible(max)
            }
            EditorRequest::GetDisableSubmit => Request::GetDisableSubmit,
            EditorRequest::SetDisableSubmit(disabled) => Request::SetDisableSubmit(disabled),
            EditorRequest::IsShowingAutocomplete => Request::IsShowingAutocomplete,
            EditorRequest::GetFocused => Request::GetFocused,
            EditorRequest::SetFocused(focused) => Request::SetFocused(focused),
        }
    }
}

impl Controlled {
    /// The owned form of a request on an editor another extension's factory built.
    fn foreign_op(
        &self,
        request: crate::component_adapter::ForeignRequest<'_, Self>,
    ) -> super::foreign_host::Op {
        use super::foreign_host::Op;
        use crate::component_adapter::{ForeignRequest as Foreign, HandlerSlot};
        let handler = |slot, handler: Option<&super::controlled::Callback>| {
            let slot = widget_editors::slot_name(slot);
            Op::SetHandler(slot, handler.map(|handler| self.callback_id(handler)))
        };
        match request {
            Foreign::GetText => Op::GetText,
            Foreign::SetText(text) => Op::Edit(text.to_owned(), false),
            Foreign::InsertTextAtCursor(text) => Op::Edit(text.to_owned(), true),
            Foreign::SetOnSubmit(callback) => handler(HandlerSlot::Submit, callback),
            Foreign::SetOnChange(callback) => handler(HandlerSlot::Change, callback),
            Foreign::OnChange => Op::OnChange,
            Foreign::AddToHistory(text) => {
                Op::Plain(widget_editors::Request::AddToHistory(text.to_owned()))
            }
            Foreign::GetExpandedText => Op::Plain(widget_editors::Request::GetExpandedText),
            Foreign::BorderColor => Op::BorderColor,
            Foreign::SetBorderColor(style) => Op::SetBorderColor(self.paint(&style)),
            Foreign::SetPaddingX(padding) => {
                Op::Plain(widget_editors::Request::SetPaddingX(padding))
            }
            Foreign::SetAutocompleteMaxVisible(max) => {
                Op::Plain(widget_editors::Request::SetAutocompleteMaxVisible(max))
            }
            Foreign::SetAutocompleteProvider(provider) => {
                Op::SetAutocompleteProvider(self.callback_id(provider))
            }
        }
    }
}

/// The adapter form of an editor answer.
fn editor_answer(answer: widget_editors::Answer) -> EditorAnswer {
    use widget_editors::Answer;
    match answer {
        Answer::Done => EditorAnswer::Done,
        Answer::Text(text) => EditorAnswer::Text(text),
        Answer::Lines(lines) => EditorAnswer::Lines(lines),
        Answer::Cursor(line, col) => EditorAnswer::Cursor(line, col),
        Answer::Number(number) => EditorAnswer::Number(number),
        Answer::Flag(flag) => EditorAnswer::Flag(flag),
    }
}

impl Controlled {
    /// The component and editor identities of the authored editor the host shows.
    fn shown_editor(&self) -> Result<(u32, u32), String> {
        let observed = self.observe();
        let slot = observed.ui.editor.as_ref().ok_or("no authored editor")?;
        Ok((slot.component, slot.editor))
    }

    /// Runs one host request on the authored editor the host shows.
    pub fn authored_editor(&self, request: serde_json::Value) -> Result<serde_json::Value, String> {
        use super::editor_host::{self as described, EditorOp};
        use crate::component_adapter::{AuthoredAnswer, AuthoredRequest};
        let request = match described::op(request)? {
            EditorOp::GetText => AuthoredRequest::GetText,
            EditorOp::SetText { text } => AuthoredRequest::SetText(text),
            EditorOp::SetOnSubmit { callback } => {
                AuthoredRequest::SetOnSubmit(callback.map(TextCallback))
            }
            EditorOp::OnChange => AuthoredRequest::OnChange,
            EditorOp::SetOnChange { callback } => {
                AuthoredRequest::SetOnChange(callback.map(TextCallback))
            }
            EditorOp::AddToHistory { text } => AuthoredRequest::AddToHistory(text),
            EditorOp::InsertTextAtCursor { text } => AuthoredRequest::InsertTextAtCursor(text),
            EditorOp::GetExpandedText => AuthoredRequest::GetExpandedText,
            EditorOp::BorderColor => AuthoredRequest::BorderColor,
            EditorOp::SetBorderColor { role } => AuthoredRequest::SetBorderColor(Style(role)),
            EditorOp::SetPaddingX { padding } => AuthoredRequest::SetPaddingX(padding),
            EditorOp::SetAutocompleteMaxVisible { max } => {
                AuthoredRequest::SetAutocompleteMaxVisible(max)
            }
            EditorOp::SetAutocompleteProvider => {
                AuthoredRequest::SetAutocompleteProvider(provider_host::Lent::Base)
            }
        };
        let (_, editor) = self.shown_editor()?;
        let emitter = super::bus_host::Emitter { key: 0 };
        let exports = crate::component_adapter::Exports::new(self.clone());
        Ok(match exports.invoke_editor(editor, request, &emitter)? {
            AuthoredAnswer::Done => described::done(),
            AuthoredAnswer::Absent => described::absent(),
            AuthoredAnswer::Text(text) => described::text(&text),
            AuthoredAnswer::Handler(None) => described::handler(false),
            AuthoredAnswer::Handler(Some(handler)) => {
                with_emitter(self, &emitter, |emitter| handler(described::PROBE, emitter))?;
                described::handler(true)
            }
            AuthoredAnswer::Style(style) => described::style(&style.apply(described::BORDER)?),
        })
    }

    /// Calls one method of the authored editor's component.
    pub fn editor_component(
        &self,
        call: super::renderer_host::Call<'_>,
    ) -> Result<super::renderer_host::Reply, String> {
        let (component, _) = self.shown_editor()?;
        super::renderer_host::invoke(&authored_component(component)?, call, self, 0)
    }
}

/// Accepts the host's own provider; the test host never calls an authored provider it lent.
fn own(provider: provider_host::Lent) -> ExtensionResult<()> {
    match provider {
        provider_host::Lent::Base => Ok(()),
        provider_host::Lent::Authored(id) => Err(format!("the test host lent provider {id}")),
    }
}

/// The authored form of the host's suggestions.
fn suggestions((items, prefix): provider_host::Found) -> crate::AutocompleteSuggestions {
    crate::AutocompleteSuggestions {
        items: items
            .into_iter()
            .map(|(value, label, description)| crate::AutocompleteItem {
                value,
                label,
                description,
            })
            .collect(),
        prefix,
    }
}

impl Controlled {
    /// Runs one host request on the authored provider on top of the stack, with a signal the
    /// driver lent.
    pub async fn provider_request(
        &self,
        request: serde_json::Value,
        flags: &[super::controlled::Flag],
    ) -> Result<serde_json::Value, String> {
        use crate::component_adapter::{ProviderAnswer, ProviderRequest};
        use provider_host::ProviderOp;
        let at = |(line, col): (u32, u32)| crate::CursorPosition { line, col };
        let (request, signal) = match provider_host::op(request)? {
            ProviderOp::Suggestions {
                lines,
                cursor,
                force,
                signal,
            } => {
                let flag = flags.get(signal).ok_or("no such signal")?;
                (
                    ProviderRequest::Suggestions(lines, at(cursor), force),
                    Some(self.signal(flag)),
                )
            }
            ProviderOp::Apply {
                lines,
                cursor,
                item: (value, label, description),
                prefix,
            } => {
                let item = crate::AutocompleteItem {
                    value,
                    label,
                    description,
                };
                (
                    ProviderRequest::Apply(lines, at(cursor), item, prefix),
                    None,
                )
            }
            ProviderOp::Trigger { lines, cursor } => {
                (ProviderRequest::Trigger(lines, at(cursor)), None)
            }
        };
        let top = provider_host::top(&self.observe())?;
        let exports = crate::component_adapter::Exports::new(self.clone());
        Ok(match exports.invoke_provider(top, request, signal).await? {
            ProviderAnswer::Suggestions(found) => provider_host::described(found.map(|found| {
                let items = found
                    .items
                    .into_iter()
                    .map(|item| (item.value, item.label, item.description));
                (items.collect(), found.prefix)
            })),
            ProviderAnswer::Applied(result) => {
                provider_host::applied(&result.lines, result.cursor_line, result.cursor_col)
            }
            ProviderAnswer::Trigger(decision) => provider_host::decided(decision),
        })
    }
}

impl crate::component_adapter::EditorImports for Controlled {
    type EditorFactory = ForeignFactory;
    fn editor_factory_build(
        &self,
        factory: &ForeignFactory,
        _: &super::controlled::Tui,
        theme: EditorStyles<'_, Self>,
        keybindings: &Keybindings,
    ) -> ExtensionResult<(Widget, bool)> {
        let custom = factory.0;
        let keys = custom.then_some(keybindings.0);
        let widget = self.build_editor(&theme, keys, crate::EditorOptions::default());
        super::foreign_host::built(&mut self.observe(), widget.0, custom);
        Ok((widget, custom))
    }
    fn foreign_editor(
        &self,
        widget: &Widget,
        request: crate::component_adapter::ForeignRequest<'_, Self>,
    ) -> ExtensionResult<crate::component_adapter::ForeignAnswer<Self>> {
        use super::foreign_host::{self as foreign, Answer};
        use crate::component_adapter::ForeignAnswer;
        let op = self.foreign_op(request);
        let answer = foreign::request(&mut self.observe(), widget.0, op)?;
        Ok(match answer {
            Answer::Done => ForeignAnswer::Done,
            Answer::Absent => ForeignAnswer::Absent,
            Answer::Text(text) => ForeignAnswer::Text(text),
            Answer::Handler(lent) => {
                ForeignAnswer::Handler(lent.then(|| TextCallback(foreign::LENT.to_owned())))
            }
            Answer::Style(role) => ForeignAnswer::Style(Style(role)),
            Answer::Calls(calls) => {
                self.calls(widget.0, calls)?;
                ForeignAnswer::Done
            }
        })
    }
    fn widget_border_color(&self, widget: &Widget) -> ExtensionResult<Style> {
        widget_editors::border_color(&mut self.observe(), widget.0).map(Style)
    }
    type TextCallback = TextCallback;
    fn text_callback(&self, callback: &TextCallback, text: &str) -> ExtensionResult<()> {
        self.observe()
            .log(format!("text {} {}", callback.0, json!(text)));
        Ok(())
    }
    fn widget_set_autocomplete_provider(
        &self,
        widget: &Widget,
        provider: &super::controlled::Callback,
    ) -> ExtensionResult<()> {
        let provider = self.callback_id(provider);
        let line = format!(
            "widget {} setAutocompleteProvider {}",
            widget.0,
            json!([provider])
        );
        self.observe().log(line);
        Ok(())
    }
    type Provider = provider_host::Lent;
    fn provider_suggestions<'a>(
        &'a self,
        provider: &'a provider_host::Lent,
        (lines, cursor, force): (&'a [String], crate::CursorPosition, Option<bool>),
        signal: &'a super::controlled::Signal,
    ) -> crate::ExtensionFuture<'a, Option<crate::AutocompleteSuggestions>> {
        let found = own(*provider).map(|()| {
            let aborted = self.aborted(signal);
            let at = (cursor.line, cursor.col);
            provider_host::suggestions(&mut self.observe(), lines, at, (force, aborted))
        });
        Box::pin(std::future::ready(
            found.map(|found| found.map(suggestions)),
        ))
    }
    fn provider_apply(
        &self,
        provider: &provider_host::Lent,
        (lines, cursor): (&[String], crate::CursorPosition),
        item: &crate::AutocompleteItem,
        prefix: &str,
    ) -> ExtensionResult<crate::CompletionResult> {
        own(*provider)?;
        let at = (cursor.line, cursor.col);
        let operands = (item.value.as_str(), prefix);
        let (lines, cursor_line, cursor_col) =
            provider_host::apply(&mut self.observe(), lines, at, operands);
        Ok(crate::CompletionResult {
            lines,
            cursor_line,
            cursor_col,
        })
    }
    fn provider_trigger(
        &self,
        provider: &provider_host::Lent,
        (lines, cursor): (&[String], crate::CursorPosition),
    ) -> ExtensionResult<Option<bool>> {
        own(*provider)?;
        let at = (cursor.line, cursor.col);
        Ok(provider_host::trigger(&mut self.observe(), lines, at))
    }
}

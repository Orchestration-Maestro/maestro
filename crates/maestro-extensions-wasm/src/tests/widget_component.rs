//! The component host's widgets: the shared fake, with nested calls handed out through
//! routes.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::collections::VecDeque;

use serde_json::json;
use wasmtime::component::Resource;

use super::host::{Identity, Route, State};
use super::ui_host::Nested;
use super::widget_host::{self, Build, CODE, Change, Child, Paint, Part, ROLES};
use super::widget_lists::{self, Call, Event, List, Setting};
use crate::bindings::host_side::maestro::extension::host;

/// Host type of one widget, by key.
pub struct WidgetResource(pub u32);

/// Host type of one host style, by role.
pub struct StyleResource(pub String);

/// Host type of one text callback lent to an authored editor, by name.
pub struct TextCallbackResource(pub String);

/// Host type of the host's code highlighting.
pub struct HighlighterResource;

/// A rendering's pieces and the results of its nested calls, in order, before its rows.
pub struct Rows {
    /// The rendering's pieces.
    parts: Vec<Part>,
    /// Results of the nested calls, in order.
    pub(super) results: VecDeque<host::LocalResult>,
    /// A failure found before the first nested call.
    failure: Option<String>,
}

impl Rows {
    /// The rows of the rendering, from its pieces and nested results.
    pub(super) fn rows(&mut self) -> Result<Vec<String>, String> {
        if let Some(error) = self.failure.take() {
            return Err(error);
        }
        let mut rows = Vec::new();
        for part in std::mem::take(&mut self.parts) {
            let result = match part {
                Part::Row(row) => {
                    rows.push(row);
                    continue;
                }
                _ => self.results.pop_front(),
            };
            match result {
                Some(host::LocalResult::Style(row)) => rows.push(row?),
                Some(host::LocalResult::Render(lines) | host::LocalResult::Highlight(lines)) => {
                    rows.extend(lines?);
                }
                Some(host::LocalResult::Widget(answer)) => match answer? {
                    host::WidgetAnswer::Text(text) => rows.push(text),
                    _ => return Err("expected text".to_owned()),
                },
                _ => return Err("a nested call has no result".to_owned()),
            }
        }
        Ok(rows)
    }
}

impl State {
    /// The paint a styling operand names.
    pub(super) fn paint(&self, style: &host::StyleSource) -> wasmtime::Result<Paint> {
        Ok(match style {
            host::StyleSource::Host(style) => Paint::Host(self.table.get(style)?.0.clone()),
            host::StyleSource::Authored(callback) => Paint::Authored(self.table.get(callback)?.0),
        })
    }

    /// The optional paint an optional styling operand names.
    pub(super) fn paints(
        &self,
        style: Option<&host::StyleSource>,
    ) -> wasmtime::Result<Option<Paint>> {
        style.map(|style| self.paint(style)).transpose()
    }

    /// Records, keeps and lends one widget.
    pub(super) fn build_widget(
        &mut self,
        build: Build,
    ) -> wasmtime::Result<Result<Resource<WidgetResource>, String>> {
        let key = widget_host::build(&mut self.observed, build);
        Ok(Ok(self.push(WidgetResource(key))?))
    }

    /// The paints of Markdown's roles in field order, and of its highlighting.
    fn markdown_paints(
        &self,
        theme: &host::MarkdownStyles,
    ) -> wasmtime::Result<(Vec<Paint>, Option<Paint>)> {
        let roles = [
            &theme.heading,
            &theme.link,
            &theme.link_url,
            &theme.code,
            &theme.code_block,
            &theme.code_block_border,
            &theme.quote,
            &theme.quote_border,
            &theme.hr,
            &theme.list_bullet,
            &theme.bold,
            &theme.italic,
            &theme.strikethrough,
            &theme.underline,
        ];
        let roles = roles
            .into_iter()
            .map(|role| self.paint(role))
            .collect::<wasmtime::Result<Vec<_>>>()?;
        let highlight = match &theme.highlight_code {
            Some(host::HighlightSource::Host(_)) => Some(Paint::Host("hl".to_owned())),
            Some(host::HighlightSource::Authored(callback)) => {
                Some(Paint::Authored(self.table.get(callback)?.0))
            }
            None => None,
        };
        Ok((roles, highlight))
    }

    /// Records a Markdown widget with the styles that name its roles.
    pub(super) fn build_markdown(
        &mut self,
        text: String,
        padding: (u32, u32),
        theme: &host::MarkdownStyles,
        style: Option<&host::DefaultTextStyle>,
    ) -> wasmtime::Result<Result<Resource<WidgetResource>, String>> {
        let (roles, highlight) = self.markdown_paints(theme)?;
        let style = match style {
            Some(style) => {
                let color = self
                    .paints(style.color.as_ref())?
                    .map(|paint| format!("{paint:?}"));
                let bg = self
                    .paints(style.bg_color.as_ref())?
                    .map(|paint| format!("{paint:?}"));
                let decorations: Vec<_> = style
                    .decorations
                    .iter()
                    .map(|decoration| match decoration {
                        host::TextDecoration::Bold => "Bold",
                        host::TextDecoration::Italic => "Italic",
                        host::TextDecoration::Strikethrough => "Strikethrough",
                        host::TextDecoration::Underline => "Underline",
                    })
                    .collect();
                Some(json!([color, bg, decorations]))
            }
            None => None,
        };
        let operands = json!({
            "roles": roles.iter().map(|role| format!("{role:?}")).collect::<Vec<_>>(),
            "highlight": highlight.as_ref().map(|paint| format!("{paint:?}")),
            "indent": theme.code_block_indent,
            "style": style,
        });
        self.build_widget(Build {
            kind: "markdown",
            text,
            padding,
            paint: roles.into_iter().next(),
            highlight,
            operands,
            ..Build::default()
        })
    }

    /// Lends the fourteen host styles and the host's highlighting.
    pub(super) fn markdown_theme(&mut self) -> wasmtime::Result<host::MarkdownTheme> {
        self.observed.log("getMarkdownTheme");
        let mut styles = ROLES
            .iter()
            .map(|role| self.push(StyleResource((*role).to_owned())))
            .collect::<wasmtime::Result<VecDeque<_>>>()?;
        let mut next = || {
            styles
                .pop_front()
                .ok_or_else(|| wasmtime::format_err!("too few roles"))
        };
        Ok(host::MarkdownTheme {
            heading: next()?,
            link: next()?,
            link_url: next()?,
            code: next()?,
            code_block: next()?,
            code_block_border: next()?,
            quote: next()?,
            quote_border: next()?,
            hr: next()?,
            list_bullet: next()?,
            bold: next()?,
            italic: next()?,
            strikethrough: next()?,
            underline: next()?,
            highlight_code: self.push(HighlighterResource)?,
        })
    }

    /// The nested call one piece of a rendering makes, if any.
    fn nested(&mut self, part: &Part, width: u32) -> wasmtime::Result<Option<host::LocalCall>> {
        Ok(match part {
            Part::Row(_) => None,
            Part::Style(target, text) => Some(host::LocalCall::Style(host::LocalStyle {
                target: *target,
                text: text.clone(),
            })),
            Part::Component(target) => Some(host::LocalCall::Render(host::LocalRender {
                target: *target,
                width,
                emitter: self.table.push(super::bus_host::Emitter { key: 0 })?,
            })),
            Part::Highlight(target) => Some(host::LocalCall::Highlight(host::LocalHighlight {
                target: *target,
                code: CODE.0.to_owned(),
                lang: Some(CODE.1.to_owned()),
            })),
            Part::Widget(target, event) => Some(self.widget_call(*target, 0, event.clone())?),
        })
    }

    /// Records one change of `this`.
    fn change(
        &mut self,
        this: &Resource<WidgetResource>,
        change: Change,
    ) -> wasmtime::Result<Result<(), String>> {
        let key = self.table.get(this)?.0;
        Ok(widget_host::change(&mut self.observed, key, change))
    }
}

/// Unsupported frame writer requests; only redraw requests are recorded here.
const NO_WRITER: &str = "the component test host lends no frame writer";

impl host::HostWidgetWithStore<State> for super::host::Imports {
    async fn drain_input(
        _: &wasmtime::component::Accessor<State, Self>,
        _: Resource<WidgetResource>,
        _: Option<f64>,
        _: Option<f64>,
    ) -> wasmtime::Result<Result<(), String>> {
        Ok(std::future::ready(Err(NO_WRITER.to_owned())).await)
    }
}

impl host::HostWidget for State {
    fn writer(
        &mut self,
        _: Resource<WidgetResource>,
        request: host::WriterRequest,
    ) -> wasmtime::Result<Resource<Route>> {
        if let host::WriterRequest::RequestRender(force) = request {
            self.observed.log(format!("writer requestRender {force}"));
            return self.routed(Ok(()));
        }
        self.routed(Err(NO_WRITER.to_owned()))
    }
    fn add_input_listener(
        &mut self,
        _: Resource<WidgetResource>,
        _: Resource<Identity>,
    ) -> wasmtime::Result<Result<Resource<super::bus_host::Subscription>, String>> {
        Ok(Err(NO_WRITER.to_owned()))
    }
    fn handle_input(
        &mut self,
        this: Resource<WidgetResource>,
        data: String,
    ) -> wasmtime::Result<Resource<Route>> {
        let key = self.table.get(&this)?.0;
        let calls = if super::widget_editors::is_editor(&self.observed, key) {
            super::widget_editors::input(&mut self.observed, key, &data)
        } else {
            widget_lists::input(&mut self.observed, key, &data)
        };
        match calls {
            Ok(calls) => self.list_route(key, calls),
            Err(error) => self.routed(Err(error)),
        }
    }
    fn editor(
        &mut self,
        this: Resource<WidgetResource>,
        request: host::EditorRequest,
    ) -> wasmtime::Result<Result<host::EditorAnswer, String>> {
        let key = self.table.get(&this)?.0;
        let request = self.editor_request(request)?;
        let answer = super::widget_editors::request(&mut self.observed, key, request);
        Ok(answer.map(super::editor_component::wire_answer))
    }
    fn set_editor_text(
        &mut self,
        this: Resource<WidgetResource>,
        text: String,
    ) -> wasmtime::Result<Resource<Route>> {
        self.edit_route(&this, &text, false)
    }
    fn insert_editor_text(
        &mut self,
        this: Resource<WidgetResource>,
        text: String,
    ) -> wasmtime::Result<Resource<Route>> {
        self.edit_route(&this, &text, true)
    }
    fn border_color(
        &mut self,
        this: Resource<WidgetResource>,
    ) -> wasmtime::Result<Result<Resource<StyleResource>, String>> {
        let key = self.table.get(&this)?.0;
        match super::widget_editors::border_color(&mut self.observed, key) {
            Ok(role) => Ok(Ok(self.push(StyleResource(role))?)),
            Err(error) => Ok(Err(error)),
        }
    }
    fn set_autocomplete_provider(
        &mut self,
        this: Resource<WidgetResource>,
        provider: Resource<Identity>,
    ) -> wasmtime::Result<Result<(), String>> {
        let key = self.table.get(&this)?.0;
        let provider = self.table.get(&provider)?.0;
        let line = format!("widget {key} setAutocompleteProvider {}", json!([provider]));
        self.observed.log(line);
        Ok(Ok(()))
    }
    fn foreign_editor(
        &mut self,
        this: Resource<WidgetResource>,
        request: host::ForeignEditorRequest,
    ) -> wasmtime::Result<Resource<Route>> {
        self.foreign_route(&this, request)
    }
    fn share(
        &mut self,
        this: Resource<WidgetResource>,
    ) -> wasmtime::Result<Resource<WidgetResource>> {
        let key = self.table.get(&this)?.0;
        self.push(WidgetResource(key))
    }
    fn on_action(
        &mut self,
        this: Resource<WidgetResource>,
        action: String,
        handler: Resource<Identity>,
    ) -> wasmtime::Result<Result<(), String>> {
        let key = self.table.get(&this)?.0;
        let handler = self.table.get(&handler)?.0;
        Ok(super::widget_editors::on_action(
            &mut self.observed,
            key,
            &action,
            handler,
        ))
    }
    fn set_filter(
        &mut self,
        this: Resource<WidgetResource>,
        filter: String,
    ) -> wasmtime::Result<Result<(), String>> {
        let key = self.table.get(&this)?.0;
        self.observed
            .log(format!("widget {key} setFilter {}", json!(filter)));
        let list = match widget_lists::list(&mut self.observed, key) {
            Ok(list) => list,
            Err(error) => return Ok(Err(error)),
        };
        let filter = filter.to_lowercase();
        list.filtered = (0..list.items.len())
            .filter(|&index| list.items[index].value.to_lowercase().starts_with(&filter))
            .collect();
        list.selected = 0;
        Ok(Ok(()))
    }
    fn set_selected_index(
        &mut self,
        this: Resource<WidgetResource>,
        index: i64,
    ) -> wasmtime::Result<Result<(), String>> {
        let key = self.table.get(&this)?.0;
        self.observed
            .log(format!("widget {key} setSelectedIndex {index}"));
        let list = match widget_lists::list(&mut self.observed, key) {
            Ok(list) => list,
            Err(error) => return Ok(Err(error)),
        };
        let final_row = i64::try_from(list.filtered.len())? - 1;
        list.selected = usize::try_from(index.min(final_row).max(0))?;
        Ok(Ok(()))
    }
    fn get_selected_item(
        &mut self,
        this: Resource<WidgetResource>,
    ) -> wasmtime::Result<Result<Option<host::SelectItem>, String>> {
        let key = self.table.get(&this)?.0;
        let list = widget_lists::list(&mut self.observed, key);
        Ok(list.map(|list| widget_lists::selected(list).map(wire_item)))
    }
    fn set_handler(
        &mut self,
        this: Resource<WidgetResource>,
        slot: host::HandlerSlot,
        handler: Option<Resource<Identity>>,
    ) -> wasmtime::Result<Result<(), String>> {
        let key = self.table.get(&this)?.0;
        let slot = super::widget_editors::slot_name(super::editor_component::slot(slot));
        let handler = handler
            .map(|handler| self.table.get(&handler).map(|identity| identity.0))
            .transpose()?;
        self.observed.log(format!(
            "widget {key} setHandler {slot} {}",
            handler.is_some()
        ));
        let handlers = if super::widget_editors::is_editor(&self.observed, key) {
            super::widget_editors::editor(&mut self.observed, key)
                .map(|editor| &mut editor.handlers)
        } else {
            widget_lists::list(&mut self.observed, key).map(|list| &mut list.handlers)
        };
        let handlers = match handlers {
            Ok(handlers) => handlers,
            Err(error) => return Ok(Err(error)),
        };
        match handler {
            Some(handler) => handlers.insert(slot, handler),
            None => handlers.remove(slot),
        };
        Ok(Ok(()))
    }
    fn update_value(
        &mut self,
        this: Resource<WidgetResource>,
        id: String,
        value: String,
    ) -> wasmtime::Result<Result<(), String>> {
        let key = self.table.get(&this)?.0;
        self.observed
            .log(format!("widget {key} updateValue {}", json!([id, value])));
        let list = match widget_lists::list(&mut self.observed, key) {
            Ok(list) => list,
            Err(error) => return Ok(Err(error)),
        };
        for setting in list.settings.iter_mut().filter(|setting| setting.id == id) {
            setting.value.clone_from(&value);
        }
        Ok(Ok(()))
    }
    fn signal(
        &mut self,
        this: Resource<WidgetResource>,
    ) -> wasmtime::Result<Result<Resource<super::host::Flag>, String>> {
        let key = self.table.get(&this)?.0;
        match widget_lists::list(&mut self.observed, key) {
            Ok(list) => {
                let flag = list.signal.clone();
                Ok(Ok(self.push(super::host::Flag(flag))?))
            }
            Err(error) => Ok(Err(error)),
        }
    }
    fn dispose(&mut self, this: Resource<WidgetResource>) -> wasmtime::Result<Result<(), String>> {
        let key = self.table.get(&this)?.0;
        self.observed.log(format!("widget {key} dispose"));
        Ok(Ok(()))
    }
    fn render(
        &mut self,
        this: Resource<WidgetResource>,
        width: u32,
    ) -> wasmtime::Result<Resource<Route>> {
        let key = self.table.get(&this)?.0;
        let (parts, failure) = match widget_host::render(&mut self.observed, key, width) {
            Ok(parts) => (parts, None),
            Err(error) => (Vec::new(), Some(error)),
        };
        let mut steps = VecDeque::new();
        for part in &parts {
            let calls = self.nested(part, width)?;
            steps.extend(calls.into_iter().map(|call| (call, Nested::Invalidate)));
        }
        self.push(Route {
            steps,
            running: None,
            result: None,
            rows: Some(Rows {
                parts,
                results: VecDeque::new(),
                failure,
            }),
        })
    }
    fn invalidate(&mut self, this: Resource<WidgetResource>) -> wasmtime::Result<Resource<Route>> {
        let key = self.table.get(&this)?.0;
        let components = match widget_host::invalidate(&mut self.observed, key) {
            Ok(components) => components,
            Err(error) => return self.routed(Err(error)),
        };
        let mut steps = VecDeque::new();
        for target in components {
            let unit = host::LocalUnit {
                target,
                data: None,
                emitter: self.table.push(super::bus_host::Emitter { key: 0 })?,
            };
            steps.push_back((host::LocalCall::Unit(unit), Nested::Invalidate));
        }
        self.push(Route {
            steps,
            running: None,
            rows: None,
            result: Some(Ok(host::RouteValue::Unit)),
        })
    }
    fn set_text(
        &mut self,
        this: Resource<WidgetResource>,
        text: String,
    ) -> wasmtime::Result<Result<(), String>> {
        self.change(&this, Change::Text(text))
    }
    fn set_background(
        &mut self,
        this: Resource<WidgetResource>,
        style: Option<host::StyleSource>,
    ) -> wasmtime::Result<Result<(), String>> {
        let paint = self.paints(style.as_ref())?;
        self.change(&this, Change::Background(paint))
    }
    fn set_lines(
        &mut self,
        this: Resource<WidgetResource>,
        lines: u32,
    ) -> wasmtime::Result<Result<(), String>> {
        self.change(&this, Change::Lines(lines))
    }
    fn add_child(
        &mut self,
        this: Resource<WidgetResource>,
        child: host::WidgetChild,
    ) -> wasmtime::Result<Result<(), String>> {
        let child = match child {
            host::WidgetChild::Widget(widget) => Child::Widget(self.table.get(&widget)?.0),
            host::WidgetChild::Component(callback) => {
                Child::Component(self.table.get::<Identity>(&callback)?.0)
            }
        };
        self.change(&this, Change::Add(child))
    }
    fn remove_child(
        &mut self,
        this: Resource<WidgetResource>,
        child: host::ChildKey,
    ) -> wasmtime::Result<Result<(), String>> {
        let child = match child {
            host::ChildKey::Widget(widget) => Child::Widget(self.table.get(&widget)?.0),
            host::ChildKey::Component(id) => Child::Component(id),
        };
        self.change(&this, Change::Remove(child))
    }
    fn clear(&mut self, this: Resource<WidgetResource>) -> wasmtime::Result<Result<(), String>> {
        self.change(&this, Change::Clear)
    }
    fn drop(&mut self, this: Resource<WidgetResource>) -> wasmtime::Result<()> {
        let key = self.table.get(&this)?.0;
        self.observed.log(format!("widget {key} dropped"));
        self.release(this)
    }
}

impl host::HostStyle for State {
    fn apply(
        &mut self,
        this: Resource<StyleResource>,
        text: String,
    ) -> wasmtime::Result<Result<String, String>> {
        let role = self.table.get(&this)?.0.clone();
        Ok(Ok(widget_host::apply(&mut self.observed, &role, &text)))
    }
    fn drop(&mut self, this: Resource<StyleResource>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

impl host::HostHighlighter for State {
    fn highlight(
        &mut self,
        _: Resource<HighlighterResource>,
        code: String,
        lang: Option<String>,
    ) -> wasmtime::Result<Result<Vec<String>, String>> {
        Ok(Ok(widget_host::highlight(
            &mut self.observed,
            &code,
            lang.as_deref(),
        )))
    }
    fn drop(&mut self, this: Resource<HighlighterResource>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

/// Host type of one host settings row style, by role.
pub struct RowStyleResource(pub String);

/// Host type of the closer of a settings list's open submenu, by the list's key.
pub struct SubmenuDoneResource(pub u32);

/// The wire form of a choice.
fn wire_item(item: crate::SelectItem) -> host::SelectItem {
    host::SelectItem {
        value: item.value,
        label: item.label,
        description: item.description,
    }
}

/// A choice from its wire form.
fn item(item: host::SelectItem) -> crate::SelectItem {
    crate::SelectItem {
        value: item.value,
        label: item.label,
        description: item.description,
    }
}

impl State {
    /// The nested call of one authored widget callback of the list under `key`.
    fn widget_call(
        &mut self,
        target: u32,
        key: u32,
        event: Event,
    ) -> wasmtime::Result<host::LocalCall> {
        let event = match event {
            Event::Select(choice) => host::WidgetEvent::Select(wire_item(choice)),
            Event::Cancel => host::WidgetEvent::Cancel,
            Event::SelectionChange(choice) => host::WidgetEvent::SelectionChange(wire_item(choice)),
            Event::Change(id, value) => host::WidgetEvent::Change((id, value)),
            Event::Abort => host::WidgetEvent::Abort,
            Event::Truncate(choice, width, is_selected) => {
                host::WidgetEvent::Truncate(host::TruncateContext {
                    text: choice.label.clone(),
                    max_width: width - 2,
                    column_width: 32,
                    item: wire_item(choice),
                    is_selected,
                })
            }
            Event::Row(text, selected) => host::WidgetEvent::Row((text, selected)),
            Event::Submenu(_, value) => {
                host::WidgetEvent::Submenu((value, self.push(SubmenuDoneResource(key))?))
            }
            Event::Text(text) => host::WidgetEvent::Text(text),
            Event::Notice => host::WidgetEvent::Notice,
            Event::Shortcut(data) => host::WidgetEvent::Shortcut(data),
        };
        Ok(host::LocalCall::Widget(host::LocalWidget {
            target,
            event,
            emitter: self.table.push(super::bus_host::Emitter { key: 0 })?,
        }))
    }

    /// The route of the nested calls an input of the list under `key` makes.
    pub(super) fn list_route(
        &mut self,
        key: u32,
        calls: Vec<Call>,
    ) -> wasmtime::Result<Resource<Route>> {
        let steps = self.steps(key, calls)?;
        self.push(Route {
            steps,
            running: None,
            rows: None,
            result: Some(Ok(host::RouteValue::Unit)),
        })
    }

    /// The nested calls of `calls` for the widget under `key`, each with what it settles.
    pub(super) fn steps(
        &mut self,
        key: u32,
        calls: Vec<Call>,
    ) -> wasmtime::Result<VecDeque<(host::LocalCall, Nested)>> {
        let mut steps = VecDeque::new();
        for call in calls {
            let step = match call {
                Call::Widget(target, Event::Shortcut(data)) => (
                    self.widget_call(target, key, Event::Shortcut(data.clone()))?,
                    Nested::Shortcut(key, data),
                ),
                Call::Widget(target, event) => {
                    let index = match &event {
                        Event::Submenu(index, _) => Some(*index),
                        _ => None,
                    };
                    (
                        self.widget_call(target, key, event)?,
                        Nested::Widget(key, index),
                    )
                }
                Call::Input(target, data) => {
                    let unit = host::LocalUnit {
                        target,
                        data: Some(data),
                        emitter: self.table.push(super::bus_host::Emitter { key: 0 })?,
                    };
                    (host::LocalCall::Unit(unit), Nested::Invalidate)
                }
            };
            steps.push_back(step);
        }
        Ok(steps)
    }

    /// Records a list widget, keeps its state and lends it.
    fn build_list(
        &mut self,
        kind: &'static str,
        operands: serde_json::Value,
        list: List,
    ) -> wasmtime::Result<Result<Resource<WidgetResource>, String>> {
        let key = widget_host::build(
            &mut self.observed,
            Build {
                kind,
                operands,
                ..Build::default()
            },
        );
        widget_lists::keep(&mut self.observed, key, list);
        Ok(Ok(self.push(WidgetResource(key))?))
    }

    /// The paint a row styling operand names.
    fn row_paint(&self, style: &host::RowStyleSource) -> wasmtime::Result<Paint> {
        Ok(match style {
            host::RowStyleSource::Host(style) => Paint::Host(self.table.get(style)?.0.clone()),
            host::RowStyleSource::Authored(callback) => {
                Paint::Authored(self.table.get(callback)?.0)
            }
        })
    }

    /// Records a selection list.
    pub(super) fn build_select_list(
        &mut self,
        items: Vec<host::SelectItem>,
        max_visible: u32,
        theme: &host::SelectListStyles,
        layout: &host::SelectLayout,
    ) -> wasmtime::Result<Result<Resource<WidgetResource>, String>> {
        let styles = [
            &theme.selected_prefix,
            &theme.selected_text,
            &theme.description,
            &theme.scroll_info,
            &theme.no_match,
        ]
        .into_iter()
        .map(|style| self.paint(style))
        .collect::<wasmtime::Result<Vec<_>>>()?;
        let truncate = layout
            .truncate_primary
            .as_ref()
            .map(|callback| self.table.get(callback).map(|identity| identity.0))
            .transpose()?;
        let items: Vec<_> = items.into_iter().map(item).collect();
        let operands = json!({ "items": items.iter().map(|item| [&item.value, &item.label])
            .collect::<Vec<_>>(), "maxVisible": max_visible,
            "styles": styles.iter().map(|style| format!("{style:?}")).collect::<Vec<_>>(),
            "min": layout.min_primary_column_width, "max": layout.max_primary_column_width,
            "truncate": truncate.is_some() });
        let list = List {
            items,
            selected_style: styles.get(1).cloned(),
            truncate,
            ..List::default()
        };
        self.build_list("select", operands, list)
    }

    /// Records a settings list.
    pub(super) fn build_settings_list(
        &mut self,
        (items, max_visible): (Vec<host::SettingItem>, u32),
        theme: &host::SettingsListStyles,
        (on_change, on_cancel): (u32, u32),
        enable_search: bool,
    ) -> wasmtime::Result<Result<Resource<WidgetResource>, String>> {
        let mut settings = Vec::new();
        for setting in items {
            let submenu = setting
                .submenu
                .map(|callback| self.table.get(&callback).map(|identity| identity.0))
                .transpose()?;
            settings.push(Setting {
                id: setting.id,
                label: setting.label,
                value: setting.current_value,
                values: setting.values,
                submenu,
            });
        }
        let rows = (self.row_paint(&theme.label)?, self.row_paint(&theme.value)?);
        let operands = json!({ "ids": settings.iter().map(|setting| &setting.id).collect::<Vec<_>>(),
            "maxVisible": max_visible, "rows": format!("{rows:?}"), "cursor": theme.cursor,
            "description": format!("{:?}", self.paint(&theme.description)?),
            "hint": format!("{:?}", self.paint(&theme.hint)?), "search": enable_search });
        let mut list = List {
            settings,
            rows: Some(rows),
            ..List::default()
        };
        list.handlers.insert("change", on_change);
        list.handlers.insert("cancel", on_cancel);
        self.build_list("settings", operands, list)
    }

    /// Records a bordered loader.
    pub(super) fn build_loader(
        &mut self,
        theme: &Resource<super::renderer_host::Theme>,
        message: String,
        cancellable: Option<bool>,
    ) -> wasmtime::Result<Result<Resource<WidgetResource>, String>> {
        let theme = self.table.get(theme)?.0.clone();
        let operands = json!({ "theme": theme, "message": message, "cancellable": cancellable });
        let list = List {
            loader: Some((message, cancellable.unwrap_or(true))),
            ..List::default()
        };
        self.build_list("loader", operands, list)
    }
}

impl host::HostRowStyle for State {
    fn apply(
        &mut self,
        this: Resource<RowStyleResource>,
        text: String,
        selected: bool,
    ) -> wasmtime::Result<Result<String, String>> {
        let role = self.table.get(&this)?.0.clone();
        self.observed
            .log(format!("rowStyle {role} {}", json!([text, selected])));
        Ok(Ok(format!("<{role} {selected}>{text}")))
    }
    fn drop(&mut self, this: Resource<RowStyleResource>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

impl host::HostSubmenuDone for State {
    fn done(
        &mut self,
        this: Resource<SubmenuDoneResource>,
        value: Option<String>,
    ) -> wasmtime::Result<Resource<Route>> {
        let key = self.table.get(&this)?.0;
        match widget_lists::done(&mut self.observed, key, value.as_deref()) {
            Ok(change) => self.list_route(key, change.into_iter().collect()),
            Err(error) => self.routed(Err(error)),
        }
    }
    fn drop(&mut self, this: Resource<SubmenuDoneResource>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

/// Bindings in their wire form.
type WireConfig = Vec<(String, Option<host::KeybindingKeys>)>;

/// Host type of one keybindings manager, by key.
pub struct KeybindingsResource(pub u32);

/// Keys from their wire form.
fn keys(keys: host::KeybindingKeys) -> crate::KeybindingKeys {
    match keys {
        host::KeybindingKeys::Single(key) => crate::KeybindingKeys::Single(key),
        host::KeybindingKeys::Multiple(keys) => crate::KeybindingKeys::Multiple(keys),
    }
}

/// The wire form of keys.
fn wire_keys(keys: crate::KeybindingKeys) -> host::KeybindingKeys {
    match keys {
        crate::KeybindingKeys::Single(key) => host::KeybindingKeys::Single(key),
        crate::KeybindingKeys::Multiple(keys) => host::KeybindingKeys::Multiple(keys),
    }
}

/// Bindings from their wire form.
pub(super) fn config(config: WireConfig) -> crate::KeybindingsConfig {
    config
        .into_iter()
        .map(|(id, bound)| (id, bound.map(keys)))
        .collect()
}

/// The wire form of bindings.
fn wire_config(config: crate::KeybindingsConfig) -> WireConfig {
    config
        .into_iter()
        .map(|(id, bound)| (id, bound.map(wire_keys)))
        .collect()
}

/// Definitions from their wire form.
pub(super) fn definitions(
    definitions: Vec<(String, host::KeybindingDefinition)>,
) -> crate::KeybindingDefinitions {
    definitions
        .into_iter()
        .map(|(id, definition)| {
            let definition = crate::KeybindingDefinition {
                default_keys: keys(definition.default_keys),
                description: definition.description,
            };
            (id, definition)
        })
        .collect()
}

impl State {
    /// Answers one request on `this`.
    fn keys_request(
        &mut self,
        this: &Resource<KeybindingsResource>,
        request: super::widget_keys::Request,
    ) -> wasmtime::Result<super::widget_keys::Answer> {
        let key = self.table.get(this)?.0;
        Ok(super::widget_keys::request(
            &mut self.observed,
            key,
            request,
        ))
    }

    /// Keeps a manager and lends it.
    pub(super) fn keep_keys(
        &mut self,
        definitions: Option<crate::KeybindingDefinitions>,
        user: crate::KeybindingsConfig,
        path: Option<String>,
    ) -> wasmtime::Result<Result<Resource<KeybindingsResource>, String>> {
        let key = super::widget_keys::keep(&mut self.observed, definitions, user, path);
        Ok(Ok(self.push(KeybindingsResource(key))?))
    }
}

impl host::HostKeybindingsManager for State {
    fn matches(
        &mut self,
        this: Resource<KeybindingsResource>,
        data: String,
        keybinding: String,
    ) -> wasmtime::Result<Result<bool, String>> {
        let request = super::widget_keys::Request::Matches(data, keybinding);
        match self.keys_request(&this, request)? {
            super::widget_keys::Answer::Matched(matched) => Ok(Ok(matched)),
            _ => Ok(Err("no match".to_owned())),
        }
    }
    fn get_keys(
        &mut self,
        this: Resource<KeybindingsResource>,
        keybinding: String,
    ) -> wasmtime::Result<Result<Vec<String>, String>> {
        match self.keys_request(&this, super::widget_keys::Request::Keys(keybinding))? {
            super::widget_keys::Answer::Keys(keys) => Ok(Ok(keys)),
            _ => Ok(Err("no keys".to_owned())),
        }
    }
    fn get_definition(
        &mut self,
        this: Resource<KeybindingsResource>,
        keybinding: String,
    ) -> wasmtime::Result<Result<Option<host::KeybindingDefinition>, String>> {
        let request = super::widget_keys::Request::Definition(keybinding);
        match self.keys_request(&this, request)? {
            super::widget_keys::Answer::Definition(definition) => {
                Ok(Ok(definition.map(|definition| {
                    host::KeybindingDefinition {
                        default_keys: wire_keys(definition.default_keys),
                        description: definition.description,
                    }
                })))
            }
            _ => Ok(Err("no definition".to_owned())),
        }
    }
    fn get_conflicts(
        &mut self,
        this: Resource<KeybindingsResource>,
    ) -> wasmtime::Result<Result<Vec<host::KeybindingConflict>, String>> {
        match self.keys_request(&this, super::widget_keys::Request::Conflicts)? {
            super::widget_keys::Answer::Conflicts(conflicts) => Ok(Ok(conflicts
                .into_iter()
                .map(|conflict| host::KeybindingConflict {
                    key: conflict.key,
                    keybindings: conflict.keybindings,
                })
                .collect())),
            _ => Ok(Err("no conflicts".to_owned())),
        }
    }
    fn set_user_bindings(
        &mut self,
        this: Resource<KeybindingsResource>,
        user_bindings: WireConfig,
    ) -> wasmtime::Result<Result<(), String>> {
        let request = super::widget_keys::Request::SetUserBindings(config(user_bindings));
        self.keys_request(&this, request).map(|_| Ok(()))
    }
    fn get_user_bindings(
        &mut self,
        this: Resource<KeybindingsResource>,
    ) -> wasmtime::Result<Result<WireConfig, String>> {
        self.keys_config(&this, super::widget_keys::Request::UserBindings)
    }
    fn get_resolved_bindings(
        &mut self,
        this: Resource<KeybindingsResource>,
    ) -> wasmtime::Result<Result<WireConfig, String>> {
        self.keys_config(&this, super::widget_keys::Request::ResolvedBindings)
    }
    fn reload(
        &mut self,
        this: Resource<KeybindingsResource>,
    ) -> wasmtime::Result<Result<(), String>> {
        self.keys_request(&this, super::widget_keys::Request::Reload)
            .map(|_| Ok(()))
    }
    fn get_effective_config(
        &mut self,
        this: Resource<KeybindingsResource>,
    ) -> wasmtime::Result<Result<WireConfig, String>> {
        self.keys_config(&this, super::widget_keys::Request::EffectiveConfig)
    }
    fn drop(&mut self, this: Resource<KeybindingsResource>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

impl State {
    /// Answers one request on `this` that answers bindings.
    fn keys_config(
        &mut self,
        this: &Resource<KeybindingsResource>,
        request: super::widget_keys::Request,
    ) -> wasmtime::Result<Result<WireConfig, String>> {
        match self.keys_request(this, request)? {
            super::widget_keys::Answer::Config(config) => Ok(Ok(wire_config(config))),
            _ => Ok(Err("no bindings".to_owned())),
        }
    }

    /// Lends the editor border and the selection list styles.
    pub(super) fn editor_theme(&mut self) -> wasmtime::Result<host::EditorTheme> {
        self.observed.log("getEditorTheme");
        self.lend_editor_theme()
    }

    /// Lends the live theme's editor styles without recording a request.
    pub(super) fn lend_editor_theme(&mut self) -> wasmtime::Result<host::EditorTheme> {
        let mut role = |name: &str| self.push(StyleResource(name.to_owned()));
        let border_color = role("borderMuted")?;
        let select_list = host::SelectListTheme {
            selected_prefix: role("selectedPrefix")?,
            selected_text: role("selectedText")?,
            description: role("description")?,
            scroll_info: role("scrollInfo")?,
            no_match: role("noMatch")?,
        };
        Ok(host::EditorTheme {
            border_color,
            select_list,
        })
    }
}

impl host::HostTextCallback for State {
    fn call(
        &mut self,
        this: Resource<TextCallbackResource>,
        text: String,
    ) -> wasmtime::Result<Resource<Route>> {
        let name = self.table.get(&this)?.0.clone();
        self.observed.log(format!("text {name} {}", json!(text)));
        self.routed(Ok(()))
    }
    fn drop(&mut self, this: Resource<TextCallbackResource>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

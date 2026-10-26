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

use super::host::{Identity, Route, Settlement, State};
use super::ui_host::Nested;
use super::widget_host::{self, Build, CODE, Change, Child, Paint, Part, ROLES};
use crate::bindings::host_side::maestro::extension::host;

/// Host type of one widget, by key.
pub struct WidgetResource(pub u32);

/// Host type of one host style, by role.
pub struct StyleResource(pub String);

/// Host type of the host's code highlighting.
pub struct HighlighterResource;

/// A rendering's nested calls, in order, before its rows.
pub struct RenderRoute {
    /// Nested calls not handed out yet.
    steps: VecDeque<host::LocalCall>,
    /// The rendering's pieces.
    parts: Vec<Part>,
    /// Results of the nested calls, in order.
    results: VecDeque<host::LocalResult>,
    /// A failure found before the first nested call.
    failure: Option<String>,
}

impl RenderRoute {
    /// The rows of the rendering, from its pieces and nested results.
    fn rows(&mut self) -> Result<Vec<String>, String> {
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
                _ => return Err("a nested call has no result".to_owned()),
            }
        }
        Ok(rows)
    }
}

impl State {
    /// The paint a styling operand names.
    fn paint(&self, style: &host::StyleSource) -> wasmtime::Result<Paint> {
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

impl host::HostWidget for State {
    fn render(
        &mut self,
        this: Resource<WidgetResource>,
        width: u32,
    ) -> wasmtime::Result<Resource<RenderRoute>> {
        let key = self.table.get(&this)?.0;
        let (parts, failure) = match widget_host::render(&mut self.observed, key, width) {
            Ok(parts) => (parts, None),
            Err(error) => (Vec::new(), Some(error)),
        };
        let mut steps = VecDeque::new();
        for part in &parts {
            steps.extend(self.nested(part, width)?);
        }
        self.push(RenderRoute {
            steps,
            parts,
            results: VecDeque::new(),
            failure,
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
            result: Some(Ok(())),
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

impl host::HostRenderRoute for State {
    fn next(&mut self, this: Resource<RenderRoute>) -> wasmtime::Result<host::RenderStep> {
        let route = self.table.get_mut(&this)?;
        if let Some(call) = route.steps.pop_front() {
            return Ok(host::RenderStep::Invoke(call));
        }
        Ok(host::RenderStep::Done(route.rows()))
    }
    fn resume(
        &mut self,
        this: Resource<RenderRoute>,
        value: host::LocalResult,
    ) -> wasmtime::Result<()> {
        self.table.get_mut(&this)?.results.push_back(value);
        Ok(())
    }
    fn defer(&mut self, _: Resource<RenderRoute>) -> wasmtime::Result<Resource<Settlement>> {
        self.push(Settlement)
    }
    fn drop(&mut self, this: Resource<RenderRoute>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

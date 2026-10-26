//! The controlled host's widgets: the shared fake, with nested calls made directly.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use serde_json::json;

use super::controlled::Controlled;
use super::widget_host::{self, Build, CODE, Change, Child, Paint, Part, ROLES};
use crate::component_adapter::renderers::with_emitter;
use crate::component_adapter::{
    ChildKey, ChildRef, HighlightRef, HostMarkdownTheme, Imports, MarkdownStyles, StyleRef,
    TextStyle, WidgetImports, authored_component, invoke_highlight, invoke_style,
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
        let component = authored_component(id)?;
        let emitter = super::bus_host::Emitter { key: 0 };
        with_emitter(self, &emitter, |emitter| method(&component, emitter))
    }
}

impl WidgetImports for Controlled {
    type Widget = Widget;
    type Style = Style;
    type Highlighter = Highlighter;

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

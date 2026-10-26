//! Widget requests through the generated host bindings.

use std::rc::Rc;

use super::{Generated, route};
use crate::bindings::maestro::extension::host;
use crate::component_adapter::{
    Builder, ChildKey, ChildRef, HighlightRef, HostMarkdownTheme, MarkdownStyles, StyleRef,
    TextStyle, WidgetImports,
};
use crate::types::ExtensionResult;

/// The host that builds this component's widgets.
pub(super) fn widget_host() -> Rc<dyn crate::WidgetHost> {
    Rc::new(Builder(Generated))
}

/// The wire form of a styling operand.
fn style<'a>(style: &StyleRef<'a, Generated>) -> host::StyleSource<'a> {
    match *style {
        StyleRef::Host(style) => host::StyleSource::Host(style),
        StyleRef::Authored(callback) => host::StyleSource::Authored(callback),
    }
}

/// The wire form of Markdown styles in field order.
fn markdown_styles(
    theme: MarkdownStyles<'_, Generated>,
) -> ExtensionResult<host::MarkdownStyles<'_>> {
    let mut roles = theme.roles.iter().map(style);
    let mut next = || {
        roles
            .next()
            .ok_or_else(|| "a Markdown style is missing".to_owned())
    };
    Ok(host::MarkdownStyles {
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
        highlight_code: theme.highlight.map(|highlight| match highlight {
            HighlightRef::Host(host) => host::HighlightSource::Host(host),
            HighlightRef::Authored(callback) => host::HighlightSource::Authored(callback),
        }),
        code_block_indent: theme.indent.map(str::to_owned),
    })
}

impl WidgetImports for Generated {
    type Widget = host::Widget;
    type Style = host::Style;
    type Highlighter = host::Highlighter;

    fn new_text(
        &self,
        text: &str,
        (padding_x, padding_y): (u32, u32),
        background: Option<StyleRef<'_, Self>>,
    ) -> ExtensionResult<host::Widget> {
        host::new_text(
            text,
            padding_x,
            padding_y,
            background.as_ref().map(style).as_ref(),
        )
    }
    fn new_box(
        &self,
        (padding_x, padding_y): (u32, u32),
        background: Option<StyleRef<'_, Self>>,
    ) -> ExtensionResult<host::Widget> {
        host::new_box(
            padding_x,
            padding_y,
            background.as_ref().map(style).as_ref(),
        )
    }
    fn new_container(&self) -> ExtensionResult<host::Widget> {
        host::new_container()
    }
    fn new_spacer(&self, lines: u32) -> ExtensionResult<host::Widget> {
        host::new_spacer(lines)
    }
    fn new_markdown(
        &self,
        text: &str,
        (padding_x, padding_y): (u32, u32),
        theme: MarkdownStyles<'_, Self>,
        text_style: Option<TextStyle<'_, Self>>,
    ) -> ExtensionResult<host::Widget> {
        let text_style = text_style.map(|text_style| host::DefaultTextStyle {
            color: text_style.color.as_ref().map(style),
            bg_color: text_style.bg_color.as_ref().map(style),
            decorations: text_style
                .decorations
                .iter()
                .map(|decoration| match decoration {
                    crate::TextDecoration::Bold => host::TextDecoration::Bold,
                    crate::TextDecoration::Italic => host::TextDecoration::Italic,
                    crate::TextDecoration::Strikethrough => host::TextDecoration::Strikethrough,
                    crate::TextDecoration::Underline => host::TextDecoration::Underline,
                })
                .collect(),
        });
        let theme = markdown_styles(theme)?;
        host::new_markdown(text, padding_x, padding_y, &theme, text_style.as_ref())
    }
    fn new_dynamic_border(
        &self,
        color: Option<StyleRef<'_, Self>>,
    ) -> ExtensionResult<host::Widget> {
        host::new_dynamic_border(color.as_ref().map(style).as_ref())
    }
    fn get_markdown_theme(&self) -> ExtensionResult<HostMarkdownTheme<Self>> {
        let theme = host::get_markdown_theme()?;
        let styles = vec![
            theme.heading,
            theme.link,
            theme.link_url,
            theme.code,
            theme.code_block,
            theme.code_block_border,
            theme.quote,
            theme.quote_border,
            theme.hr,
            theme.list_bullet,
            theme.bold,
            theme.italic,
            theme.strikethrough,
            theme.underline,
        ];
        Ok((styles, theme.highlight_code))
    }
    fn widget_render(&self, widget: &host::Widget, width: u32) -> ExtensionResult<Vec<String>> {
        route::drive_render(&widget.render(width))
    }
    fn widget_invalidate(&self, widget: &host::Widget) -> ExtensionResult<()> {
        route::drive(&widget.invalidate())
    }
    fn widget_set_text(&self, widget: &host::Widget, text: &str) -> ExtensionResult<()> {
        widget.set_text(text)
    }
    fn widget_set_background(
        &self,
        widget: &host::Widget,
        background: Option<StyleRef<'_, Self>>,
    ) -> ExtensionResult<()> {
        widget.set_background(background.as_ref().map(style).as_ref())
    }
    fn widget_set_lines(&self, widget: &host::Widget, lines: u32) -> ExtensionResult<()> {
        widget.set_lines(lines)
    }
    fn widget_add_child(
        &self,
        widget: &host::Widget,
        child: ChildRef<'_, Self>,
    ) -> ExtensionResult<()> {
        widget.add_child(&match child {
            ChildRef::Widget(child) => host::WidgetChild::Widget(child),
            ChildRef::Component(callback) => host::WidgetChild::Component(callback),
        })
    }
    fn widget_remove_child(
        &self,
        widget: &host::Widget,
        child: ChildKey<'_, Self>,
    ) -> ExtensionResult<()> {
        widget.remove_child(&match child {
            ChildKey::Widget(child) => host::ChildKey::Widget(child),
            ChildKey::Component(id) => host::ChildKey::Component(id),
        })
    }
    fn widget_clear(&self, widget: &host::Widget) -> ExtensionResult<()> {
        widget.clear()
    }
    fn style_apply(&self, style: &host::Style, text: &str) -> ExtensionResult<String> {
        style.apply(text)
    }
    fn highlight(
        &self,
        highlighter: &host::Highlighter,
        code: &str,
        lang: Option<&str>,
    ) -> ExtensionResult<Vec<String>> {
        highlighter.highlight(code, lang)
    }
}

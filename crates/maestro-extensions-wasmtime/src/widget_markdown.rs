//! Markdown styles a host widget calls, from the guest's operands and the lent theme.
use crate::guest_calls::paints::{Failure, HighlightFn};
use crate::local_resources::Registry;
use crate::widget_reads::{BaseStyle, Highlighting, Paint, Styles};
use maestro_extensions::widgets::{BorderColor, DefaultTextStyle, MarkdownTheme};
use std::rc::Rc;

/// Markdown styles from their operands.
pub(crate) fn markdown_theme(
    registry: &Registry,
    styles: Styles,
    failure: &Failure,
) -> Result<MarkdownTheme, String> {
    let mut roles = styles.roles.into_iter().map(|paint| {
        let style = registry.paint(paint, failure)?;
        Ok::<_, String>(Box::new(move |text: &str| style(text)) as Box<dyn Fn(&str) -> String>)
    });
    let mut next = || {
        roles
            .next()
            .unwrap_or_else(|| Err("a Markdown style is missing".to_owned()))
    };
    let highlight = match styles.highlight {
        Some(Highlighting::Host(highlighter)) => Some(registry.highlighter(&highlighter)?),
        Some(Highlighting::Authored(handle)) => {
            Some(handle.highlight(registry.clone(), failure.clone()))
        }
        None => None,
    };
    Ok(MarkdownTheme {
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
        highlight_code: highlight.map(|highlight| {
            Box::new(move |code: &str, lang: Option<&str>| highlight(code, lang))
                as Box<HighlightFn>
        }),
        code_block_indent: styles.indent,
    })
}

/// Markdown's base style from its operands.
pub(crate) fn base_style(
    registry: &Registry,
    base: BaseStyle,
    failure: &Failure,
) -> Result<DefaultTextStyle, String> {
    let boxed = |paint: Option<Paint>| {
        paint
            .map(|paint| {
                let style: BorderColor = registry.paint(paint, failure)?;
                Ok::<_, String>(
                    Box::new(move |text: &str| style(text)) as Box<dyn Fn(&str) -> String>
                )
            })
            .transpose()
    };
    Ok(DefaultTextStyle {
        color: boxed(base.color)?,
        bg_color: boxed(base.bg)?,
        decorations: base.decorations,
    })
}

/// The style the toolkit calls for one role of a shared Markdown theme.
pub(crate) fn role(theme: &Rc<MarkdownTheme>, index: usize) -> BorderColor {
    let theme = theme.clone();
    Rc::new(move |text: &str| {
        let roles: [&dyn Fn(&str) -> String; 14] = [
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
        roles
            .get(index)
            .map_or_else(|| text.to_owned(), |role| role(text))
    })
}

/// The highlighting the toolkit calls for a shared Markdown theme; without one, code rows stay
/// as they are.
pub(crate) fn highlighting(theme: &Rc<MarkdownTheme>) -> Rc<HighlightFn> {
    let theme = theme.clone();
    Rc::new(
        move |code: &str, lang: Option<&str>| match &theme.highlight_code {
            Some(highlight) => highlight(code, lang),
            None => code.lines().map(str::to_owned).collect(),
        },
    )
}

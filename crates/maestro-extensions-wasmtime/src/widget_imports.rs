//! Canonical widget imports delegate to the host's widgets through the local request pump.
use crate::{
    callback_imports::CallbackView,
    guest_calls::Callback,
    import_route::{Route, push},
    local_resources::Message,
    widget_reads::{
        BaseStyle, Build, Child, HighlighterEndpoint, Highlighting, Key, Paint, Request,
        StyleEndpoint, Styles, WidgetEndpoint, WidgetMessage,
    },
};
use wasmtime::{
    StoreContextMut,
    component::{ComponentType, Lift, LinkerInstance, Lower, Resource},
};

/// A styling operand: a lent host style, or an authored callback.
#[derive(ComponentType, Lift)]
#[component(variant)]
pub(crate) enum StyleSource {
    /// A lent host style.
    #[component(name = "host")]
    Host(Resource<StyleEndpoint>),
    /// An authored style.
    #[component(name = "authored")]
    Authored(Resource<Callback>),
}

/// A highlighting operand: the lent host highlighting, or an authored callback.
#[derive(ComponentType, Lift)]
#[component(variant)]
enum HighlightSource {
    /// The lent host highlighting.
    #[component(name = "host")]
    Host(Resource<HighlighterEndpoint>),
    /// Authored highlighting.
    #[component(name = "authored")]
    Authored(Resource<Callback>),
}

/// Markdown element styles as the guest sends them.
#[derive(ComponentType, Lift)]
#[component(record)]
struct MarkdownStyles {
    /// Headings.
    heading: StyleSource,
    /// Link labels.
    link: StyleSource,
    /// Link destinations.
    #[component(name = "link-url")]
    link_url: StyleSource,
    /// Inline code.
    code: StyleSource,
    /// Code rows.
    #[component(name = "code-block")]
    code_block: StyleSource,
    /// Code fences.
    #[component(name = "code-block-border")]
    code_block_border: StyleSource,
    /// Quoted text.
    quote: StyleSource,
    /// Quote borders.
    #[component(name = "quote-border")]
    quote_border: StyleSource,
    /// Rules.
    hr: StyleSource,
    /// List markers.
    #[component(name = "list-bullet")]
    list_bullet: StyleSource,
    /// Bold text.
    bold: StyleSource,
    /// Italic text.
    italic: StyleSource,
    /// Struck-through text.
    strikethrough: StyleSource,
    /// Underlined text.
    underline: StyleSource,
    /// Code highlighting.
    #[component(name = "highlight-code")]
    highlight_code: Option<HighlightSource>,
    /// Code row prefix.
    #[component(name = "code-block-indent")]
    code_block_indent: Option<String>,
}

/// Markdown's base style as the guest sends it.
#[derive(ComponentType, Lift)]
#[component(record)]
struct DefaultTextStyle {
    /// Foreground.
    color: Option<StyleSource>,
    /// Row background.
    #[component(name = "bg-color")]
    bg_color: Option<StyleSource>,
    /// Decorations to apply.
    decorations: Vec<TextDecoration>,
}

/// A text decoration of Markdown's base style.
#[derive(Clone, Copy, ComponentType, Lift)]
#[component(enum)]
#[repr(u8)]
#[expect(
    dead_code,
    reason = "constructed by the Wasmtime Lift macro from wire values; host code only matches them"
)]
enum TextDecoration {
    /// Bold text.
    #[component(name = "bold")]
    Bold,
    /// Italic text.
    #[component(name = "italic")]
    Italic,
    /// Struck-through text.
    #[component(name = "strikethrough")]
    Strikethrough,
    /// Underlined text.
    #[component(name = "underline")]
    Underline,
}

impl From<TextDecoration> for maestro_extensions::widgets::TextDecoration {
    fn from(decoration: TextDecoration) -> Self {
        match decoration {
            TextDecoration::Bold => Self::Bold,
            TextDecoration::Italic => Self::Italic,
            TextDecoration::Strikethrough => Self::Strikethrough,
            TextDecoration::Underline => Self::Underline,
        }
    }
}

/// A child to add as the guest sends it.
#[derive(ComponentType, Lift)]
#[component(variant)]
enum WidgetChild {
    /// A host widget.
    #[component(name = "widget")]
    Widget(Resource<WidgetEndpoint>),
    /// An authored component.
    #[component(name = "component")]
    Component(Resource<Callback>),
}

/// A child to remove as the guest sends it.
#[derive(ComponentType, Lift)]
#[component(variant)]
enum ChildKey {
    /// A host widget.
    #[component(name = "widget")]
    Widget(Resource<WidgetEndpoint>),
    /// The identity an authored component was added under.
    #[component(name = "component")]
    Component(u32),
}

/// The lent Markdown styles in field order, and the lent highlighting.
#[derive(ComponentType, Lower)]
#[component(record)]
struct MarkdownTheme {
    /// Headings.
    heading: Resource<StyleEndpoint>,
    /// Link labels.
    link: Resource<StyleEndpoint>,
    /// Link destinations.
    #[component(name = "link-url")]
    link_url: Resource<StyleEndpoint>,
    /// Inline code.
    code: Resource<StyleEndpoint>,
    /// Code rows.
    #[component(name = "code-block")]
    code_block: Resource<StyleEndpoint>,
    /// Code fences.
    #[component(name = "code-block-border")]
    code_block_border: Resource<StyleEndpoint>,
    /// Quoted text.
    quote: Resource<StyleEndpoint>,
    /// Quote borders.
    #[component(name = "quote-border")]
    quote_border: Resource<StyleEndpoint>,
    /// Rules.
    hr: Resource<StyleEndpoint>,
    /// List markers.
    #[component(name = "list-bullet")]
    list_bullet: Resource<StyleEndpoint>,
    /// Bold text.
    bold: Resource<StyleEndpoint>,
    /// Italic text.
    italic: Resource<StyleEndpoint>,
    /// Struck-through text.
    strikethrough: Resource<StyleEndpoint>,
    /// Underlined text.
    underline: Resource<StyleEndpoint>,
    /// Code highlighting.
    #[component(name = "highlight-code")]
    highlight_code: Resource<HighlighterEndpoint>,
}

/// The paint a styling operand names, retaining an authored style's identity.
pub(crate) fn paint<T: CallbackView>(
    store: &mut T,
    style: &StyleSource,
) -> wasmtime::Result<Paint> {
    Ok(match style {
        StyleSource::Host(style) => Paint::Host(store.ctx().table.get(style)?.clone()),
        StyleSource::Authored(callback) => {
            Paint::Authored(crate::callback_imports::retain(store, callback)?)
        }
    })
}

/// The optional paint an optional styling operand names.
fn paints<T: CallbackView>(
    store: &mut T,
    style: Option<&StyleSource>,
) -> wasmtime::Result<Option<Paint>> {
    style.map(|style| paint(store, style)).transpose()
}

/// Markdown styles from their operands.
fn styles<T: CallbackView>(store: &mut T, theme: &MarkdownStyles) -> wasmtime::Result<Styles> {
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
        .map(|role| paint(store, role))
        .collect::<wasmtime::Result<_>>()?;
    let highlight = match &theme.highlight_code {
        Some(HighlightSource::Host(host)) => {
            Some(Highlighting::Host(store.ctx().table.get(host)?.clone()))
        }
        Some(HighlightSource::Authored(callback)) => Some(Highlighting::Authored(
            crate::callback_imports::retain(store, callback)?,
        )),
        None => None,
    };
    Ok(Styles {
        roles,
        highlight,
        indent: theme.code_block_indent.clone(),
    })
}

/// Builds one widget on the local owner and lends it.
pub(crate) fn build<T: CallbackView>(
    store: &mut T,
    build: Build,
) -> wasmtime::Result<Result<Resource<WidgetEndpoint>, String>> {
    let widget = store
        .api()
        .0
        .request(|id, reply| Message::Widgets(WidgetMessage::New(id, build, reply)));
    Ok(match widget {
        Ok(widget) => Ok(store.ctx().table.push(widget)?),
        Err(error) => Err(error),
    })
}

/// Installs the widget constructors, methods and the lent styles.
pub(crate) fn add_to_linker<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    crate::context_imports::add_resource::<T, WidgetEndpoint>(host, "widget")?;
    crate::context_imports::add_resource::<T, StyleEndpoint>(host, "style")?;
    crate::context_imports::add_resource::<T, HighlighterEndpoint>(host, "highlighter")?;
    host.func_wrap(
        "[method]tui.full-redraws",
        |mut store: StoreContextMut<'_, T>,
         (this,): (Resource<crate::local_resources::effects::TuiEndpoint>,)| {
            let tui = store.data_mut().ctx().table.get(&this)?.clone();
            Ok((tui.full_redraws(),))
        },
    )?;
    crate::theme_imports::add_global(host)?;
    crate::overlay_imports::add_to_linker(host)?;
    crate::frame_writer_imports::add_to_linker(host)?;
    constructors(host)?;
    markdown(host)?;
    methods(host)?;
    children(host)?;
    crate::widget_list_imports::add_to_linker(host)?;
    crate::keys_imports::add_to_linker(host)?;
    crate::widget_editor_imports::add_to_linker(host)?;
    crate::completion_imports::add_to_linker(host)?;
    crate::foreign_editor_imports::add_to_linker(host)?;
    lent(host)
}

/// Installs the constructors of text, boxes, containers, spacers and rules.
fn constructors<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    host.func_wrap(
        "new-text",
        |mut store: StoreContextMut<'_, T>,
         (text, x, y, bg): (String, u32, u32, Option<StyleSource>)| {
            let bg = paints(store.data_mut(), bg.as_ref())?;
            Ok((build(store.data_mut(), Build::Text(text, (x, y), bg))?,))
        },
    )?;
    host.func_wrap(
        "new-box",
        |mut store: StoreContextMut<'_, T>, (x, y, bg): (u32, u32, Option<StyleSource>)| {
            let bg = paints(store.data_mut(), bg.as_ref())?;
            Ok((build(store.data_mut(), Build::Box((x, y), bg))?,))
        },
    )?;
    host.func_wrap(
        "new-container",
        |mut store: StoreContextMut<'_, T>, (): ()| {
            Ok((build(store.data_mut(), Build::Container)?,))
        },
    )?;
    host.func_wrap(
        "new-spacer",
        |mut store: StoreContextMut<'_, T>, (lines,): (u32,)| {
            Ok((build(store.data_mut(), Build::Spacer(lines))?,))
        },
    )?;
    host.func_wrap(
        "new-dynamic-border",
        |mut store: StoreContextMut<'_, T>, (color,): (Option<StyleSource>,)| {
            let color = paints(store.data_mut(), color.as_ref())?;
            Ok((build(store.data_mut(), Build::Border(color))?,))
        },
    )
}

/// Installs Markdown construction and the lent Markdown theme.
fn markdown<T: CallbackView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    host.func_wrap(
        "new-markdown",
        |mut store: StoreContextMut<'_, T>,
         (text, x, y, theme, base): (
            String,
            u32,
            u32,
            MarkdownStyles,
            Option<DefaultTextStyle>,
        )| {
            let theme = styles(store.data_mut(), &theme)?;
            let base = match base {
                Some(base) => Some(BaseStyle {
                    color: paints(store.data_mut(), base.color.as_ref())?,
                    bg: paints(store.data_mut(), base.bg_color.as_ref())?,
                    decorations: base.decorations.into_iter().map(Into::into).collect(),
                }),
                None => None,
            };
            let build_markdown = Build::Markdown(text, (x, y), theme, base);
            Ok((build(store.data_mut(), build_markdown)?,))
        },
    )?;
    host.func_wrap(
        "get-markdown-theme",
        |mut store: StoreContextMut<'_, T>, (): ()| {
            let lent = store
                .data()
                .api()
                .0
                .request(|_, reply| Message::Widgets(WidgetMessage::LendTheme(reply)));
            let lent = match lent {
                Ok((styles, highlighter)) => Ok(lend(store.data_mut(), styles, highlighter)?),
                Err(error) => Err(error),
            };
            Ok((lent,))
        },
    )
}

/// Lends the Markdown styles in field order and the highlighting.
fn lend<T: CallbackView>(
    store: &mut T,
    styles: Vec<StyleEndpoint>,
    highlighter: HighlighterEndpoint,
) -> wasmtime::Result<MarkdownTheme> {
    let table = &mut *store.ctx().table;
    let mut styles = styles.into_iter();
    let mut next = || -> wasmtime::Result<Resource<StyleEndpoint>> {
        let style = styles
            .next()
            .ok_or_else(|| wasmtime::format_err!("the host lent too few Markdown styles"))?;
        Ok(table.push(style)?)
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
        highlight_code: table.push(highlighter)?,
    })
}

/// The widget behind a borrowed resource.
pub(crate) fn widget<T: CallbackView>(
    store: &mut T,
    resource: &Resource<WidgetEndpoint>,
) -> wasmtime::Result<WidgetEndpoint> {
    Ok(store.ctx().table.get(resource)?.clone())
}

/// Installs rendering, invalidation and the text, background and row mutations.
fn methods<T: CallbackView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    host.func_wrap(
        "[method]widget.render",
        |mut store: StoreContextMut<'_, T>, (this, width): (Resource<WidgetEndpoint>, u32)| {
            let route: Route =
                widget(store.data_mut(), &this)?.route(|reply| Request::Render(width, reply));
            Ok((push(store.data_mut(), route)?,))
        },
    )?;
    host.func_wrap(
        "[method]widget.invalidate",
        |mut store: StoreContextMut<'_, T>, (this,): (Resource<WidgetEndpoint>,)| {
            let route: Route = widget(store.data_mut(), &this)?.route(Request::Invalidate);
            Ok((push(store.data_mut(), route)?,))
        },
    )?;
    host.func_wrap(
        "[method]widget.set-text",
        |mut store: StoreContextMut<'_, T>, (this, text): (Resource<WidgetEndpoint>, String)| {
            Ok((widget(store.data_mut(), &this)?.read(|reply| Request::SetText(text, reply)),))
        },
    )?;
    host.func_wrap(
        "[method]widget.set-background",
        |mut store: StoreContextMut<'_, T>,
         (this, bg): (Resource<WidgetEndpoint>, Option<StyleSource>)| {
            let bg = paints(store.data_mut(), bg.as_ref())?;
            Ok(
                (
                    widget(store.data_mut(), &this)?
                        .read(|reply| Request::SetBackground(bg, reply)),
                ),
            )
        },
    )?;
    host.func_wrap(
        "[method]widget.set-lines",
        |mut store: StoreContextMut<'_, T>, (this, lines): (Resource<WidgetEndpoint>, u32)| {
            Ok((widget(store.data_mut(), &this)?.read(|reply| Request::SetLines(lines, reply)),))
        },
    )
}

/// Installs the child operations of containers and boxes.
fn children<T: CallbackView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    host.func_wrap(
        "[method]widget.add-child",
        |mut store: StoreContextMut<'_, T>,
         (this, child): (Resource<WidgetEndpoint>, WidgetChild)| {
            let child = match child {
                WidgetChild::Widget(child) => Child::Widget(widget(store.data_mut(), &child)?),
                WidgetChild::Component(callback) => Child::Component(
                    crate::callback_imports::retain(store.data_mut(), &callback)?,
                ),
            };
            Ok((widget(store.data_mut(), &this)?.read(|reply| Request::Add(child, reply)),))
        },
    )?;
    host.func_wrap(
        "[method]widget.remove-child",
        |mut store: StoreContextMut<'_, T>, (this, key): (Resource<WidgetEndpoint>, ChildKey)| {
            let key = match key {
                ChildKey::Widget(child) => Key::Widget(widget(store.data_mut(), &child)?),
                ChildKey::Component(id) => Key::Component(id),
            };
            Ok((widget(store.data_mut(), &this)?.read(|reply| Request::Remove(key, reply)),))
        },
    )?;
    host.func_wrap(
        "[method]widget.clear",
        |mut store: StoreContextMut<'_, T>, (this,): (Resource<WidgetEndpoint>,)| {
            Ok((widget(store.data_mut(), &this)?.read(Request::Clear),))
        },
    )
}

/// Installs the methods of the lent styles and highlighting.
fn lent<T: CallbackView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    host.func_wrap(
        "[method]style.apply",
        |mut store: StoreContextMut<'_, T>, (this, text): (Resource<StyleEndpoint>, String)| {
            let style = store.data_mut().ctx().table.get(&this)?.clone();
            Ok((style.apply(text),))
        },
    )?;
    host.func_wrap(
        "[method]highlighter.highlight",
        |mut store: StoreContextMut<'_, T>,
         (this, code, lang): (Resource<HighlighterEndpoint>, String, Option<String>)| {
            let highlighter = store.data_mut().ctx().table.get(&this)?.clone();
            Ok((highlighter.highlight(code, lang),))
        },
    )
}

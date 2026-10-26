//! Host widgets an extension built, served on the local owner over the toolkit widgets.
use crate::guest_calls::Handle;
use crate::guest_calls::paints::{Failure, HighlightFn};
use crate::local_resources::{Lease, Message, Registry};
use maestro_extensions::widgets::{
    BorderColor, Box as Boxed, Container, DefaultTextStyle, ExtensionWidgets, Markdown,
    MarkdownOptions, MarkdownTheme, Spacer, Text, TextDecoration,
};
use maestro_extensions::{ComponentHandle, EventBus, Local};
use std::{cell::RefCell, rc::Rc, sync::Arc, sync::mpsc::Sender};

/// Catchable completion of one widget request.
pub(crate) type Reply<T> = Sender<Result<T, String>>;

/// A styling function a widget calls: a lent host style, or an authored one.
pub(crate) enum Paint {
    /// A host style the extension holds.
    Host(StyleEndpoint),
    /// An authored style.
    Authored(Handle),
}

/// Code highlighting a Markdown widget calls: the lent host highlighting, or authored.
pub(crate) enum Highlighting {
    /// The host's highlighting the extension holds.
    Host(HighlighterEndpoint),
    /// Authored highlighting.
    Authored(Handle),
}

/// Markdown element styles in field order, highlighting and code row prefix.
pub(crate) struct Styles {
    /// The fourteen element styles.
    pub(crate) roles: Vec<Paint>,
    /// Code highlighting.
    pub(crate) highlight: Option<Highlighting>,
    /// Code row prefix.
    pub(crate) indent: Option<String>,
}

/// Markdown's base style.
pub(crate) struct BaseStyle {
    /// Foreground.
    pub(crate) color: Option<Paint>,
    /// Row background.
    pub(crate) bg: Option<Paint>,
    /// Decorations to apply.
    pub(crate) decorations: Vec<TextDecoration>,
}

/// What a new widget is built from.
pub(crate) enum Build {
    /// Wrapped text with padding and background.
    Text(String, (u32, u32), Option<Paint>),
    /// A box with padding and background.
    Box((u32, u32), Option<Paint>),
    /// An empty container.
    Container,
    /// Blank rows.
    Spacer(u32),
    /// Markdown with padding, styles and base style.
    Markdown(String, (u32, u32), Styles, Option<BaseStyle>),
    /// A rule with its color.
    Border(Option<Paint>),
    /// A selection list, settings list or loader.
    List(crate::widget_lists::ListBuild),
}

/// A child to add.
pub(crate) enum Child {
    /// A host widget.
    Widget(WidgetEndpoint),
    /// An authored component under its identity.
    Component(Handle),
}

/// A child to remove.
pub(crate) enum Key {
    /// A host widget.
    Widget(WidgetEndpoint),
    /// The identity an authored component was added under.
    Component(u32),
}

/// One request on a widget.
pub(crate) enum Request {
    /// Renders rows at a width.
    Render(u32, Reply<Vec<String>>),
    /// Drops cached rows.
    Invalidate(Reply<()>),
    /// Replaces the text.
    SetText(String, Reply<()>),
    /// Replaces or removes the background.
    SetBackground(Option<Paint>, Reply<()>),
    /// Replaces the row count.
    SetLines(u32, Reply<()>),
    /// Appends a child.
    Add(Child, Reply<()>),
    /// Removes the first occurrence of a child.
    Remove(Key, Reply<()>),
    /// Removes every child.
    Clear(Reply<()>),
    /// Delivers input.
    Input(String, Reply<()>),
    /// A request only a list or loader takes.
    List(crate::widget_lists::ListRequest),
}

/// A message for the widgets of one extension.
pub(crate) enum WidgetMessage {
    /// A request on the widget with this identity.
    Request(u64, Request),
    /// Builds a widget for the extension whose API has this identity.
    New(u64, Build, Reply<WidgetEndpoint>),
    /// A request on the lent style or highlighting with this identity.
    Lent(u64, Lent),
    /// Lends the live theme's Markdown styles and highlighting.
    LendTheme(Reply<LentTheme>),
    /// Lends the live theme's selection list styles in field order.
    LendSelect(Reply<Vec<StyleEndpoint>>),
    /// Lends the live theme's settings list styles.
    LendSettings(Reply<LentSettings>),
    /// Keeps a new keybindings manager.
    NewKeys(
        crate::keys_reads::KeysBuild,
        Reply<crate::keys_reads::KeysEndpoint>,
    ),
    /// A request on the keybindings manager with this identity.
    Keys(u64, crate::keys_reads::KeysRequest),
    /// A keybinding hint.
    Hint(crate::keys_reads::Hint, Reply<String>),
    /// Lends the live theme's editor border and selection list styles in field order.
    LendEditor(Reply<(StyleEndpoint, Vec<StyleEndpoint>)>),
}

/// The lent settings list styles: label and value row styles, description, cursor and hint.
pub(crate) type LentSettings = (
    crate::widget_lists::RowStyleEndpoint,
    crate::widget_lists::RowStyleEndpoint,
    StyleEndpoint,
    String,
    StyleEndpoint,
);

/// One request on a lent host style or highlighting.
pub(crate) enum Lent {
    /// Applies the style.
    Style(String, Reply<String>),
    /// Runs the highlighting.
    Highlight(String, Option<String>, Reply<Vec<String>>),
    /// Applies a row style to text of a row that is selected or not.
    Row(String, bool, Reply<String>),
    /// Closes a submenu with its chosen value, or without one.
    Done(Option<String>, Reply<()>),
}

impl Lent {
    /// Returns a catchable failure.
    pub(crate) fn reject(self, error: &str) {
        let error = error.to_owned();
        match self {
            Self::Style(_, reply) | Self::Row(_, _, reply) => drop(reply.send(Err(error))),
            Self::Highlight(_, _, reply) => drop(reply.send(Err(error))),
            Self::Done(_, reply) => drop(reply.send(Err(error))),
        }
    }
}

/// The toolkit widget behind one host widget.
enum Kind {
    /// Wrapped text.
    Text(Rc<Text>),
    /// A padded box.
    Box(Rc<Boxed>),
    /// A container.
    Container(Rc<Container>),
    /// Blank rows.
    Spacer(Rc<Spacer>),
    /// Markdown.
    Markdown(Rc<Markdown>),
    /// A widget with no further operations.
    Plain(ComponentHandle),
    /// A selection list, settings list or loader.
    List(crate::widget_lists::ListKind),
}

/// One host widget, the failure its authored functions report and its authored children.
pub(crate) struct Widget {
    /// The toolkit widget.
    kind: Kind,
    /// The first failure of an authored function since the last rendering.
    failure: Failure,
    /// The extension's bus, whose frames enter authored children.
    events: EventBus<Local>,
    /// Authored children by the identity each was added under, in insertion order.
    guests: RefCell<Vec<(u32, ComponentHandle)>>,
}

impl Widget {
    /// The widget as a toolkit component.
    pub(crate) fn component(&self) -> ComponentHandle {
        match &self.kind {
            Kind::Text(widget) => widget.clone(),
            Kind::Box(widget) => widget.clone(),
            Kind::Container(widget) => widget.clone(),
            Kind::Spacer(widget) => widget.clone(),
            Kind::Markdown(widget) => widget.clone(),
            Kind::Plain(widget) => widget.clone(),
            Kind::List(list) => list.component(),
        }
    }

    /// Builds the toolkit widget `build` names.
    pub(crate) fn build(
        build: Build,
        registry: &Registry,
        widgets: &dyn ExtensionWidgets,
        events: EventBus<Local>,
    ) -> Result<Self, String> {
        let failure = Failure::default();
        let paint = |paint: Option<Paint>| {
            paint
                .map(|paint| registry.paint(paint, &failure))
                .transpose()
        };
        let kind = match build {
            Build::Text(text, (x, y), bg) => {
                Kind::Text(Rc::new(Text::new(text, width(x)?, width(y)?, paint(bg)?)))
            }
            Build::Box((x, y), bg) => {
                Kind::Box(Rc::new(Boxed::new(width(x)?, width(y)?, paint(bg)?)))
            }
            Build::Container => Kind::Container(Rc::new(Container::new())),
            Build::Spacer(lines) => Kind::Spacer(Rc::new(Spacer::new(width(lines)?))),
            Build::Border(color) => Kind::Plain(widgets.dynamic_border(paint(color)?)),
            Build::List(list) => {
                let binding = crate::widget_lists::Binding {
                    registry,
                    events: &events,
                    failure: &failure,
                };
                Kind::List(crate::widget_lists::ListKind::build(
                    list, &binding, widgets,
                )?)
            }
            Build::Markdown(text, padding, styles, base) => {
                let theme = markdown_theme(registry, styles, &failure)?;
                let options = MarkdownOptions {
                    clean_content: false,
                    padding_x: width(padding.0)?,
                    padding_y: width(padding.1)?,
                    default_text_style: base
                        .map(|base| base_style(registry, base, &failure))
                        .transpose()?,
                };
                Kind::Markdown(widgets.markdown(text, options, Rc::new(theme)))
            }
        };
        Ok(Self {
            kind,
            failure,
            events,
            guests: RefCell::default(),
        })
    }

    /// Answers one request; native code runs without a registry borrow.
    pub(crate) fn answer(&self, request: Request, registry: &Registry) {
        match request {
            Request::Render(width, reply) => {
                let rows = usize::try_from(width).map_err(|error| error.to_string());
                let rows = rows.and_then(|width| {
                    let rows = self
                        .component()
                        .try_render(width)
                        .map_err(|error| error.to_string());
                    self.failure.borrow_mut().take().map_or(rows, Err)
                });
                let _ = reply.send(rows);
            }
            Request::Invalidate(reply) => {
                self.component().invalidate();
                let _ = reply.send(self.failure.borrow_mut().take().map_or(Ok(()), Err));
            }
            Request::SetText(text, reply) => {
                let _ = reply.send(match &self.kind {
                    Kind::Text(widget) => {
                        widget.set_text(text);
                        Ok(())
                    }
                    Kind::Markdown(widget) => {
                        widget.set_text(text);
                        Ok(())
                    }
                    _ => Err("the widget has no text".to_owned()),
                });
            }
            Request::SetBackground(bg, reply) => {
                let _ = reply.send(self.background(bg, registry));
            }
            Request::SetLines(lines, reply) => {
                let _ = reply.send(match &self.kind {
                    Kind::Spacer(widget) => width(lines).map(|lines| widget.set_lines(lines)),
                    _ => Err("the widget has no rows".to_owned()),
                });
            }
            Request::Input(data, reply) => {
                let _ = reply.send(self.input(&data));
            }
            Request::List(request) => match &self.kind {
                Kind::List(list) => {
                    let binding = crate::widget_lists::Binding {
                        registry,
                        events: &self.events,
                        failure: &self.failure,
                    };
                    list.answer(request, &binding);
                }
                _ => request.reject("the widget is no list"),
            },
            request => self.children(request, registry),
        }
    }

    /// Delivers input, then reports an authored callback's failure.
    fn input(&self, data: &str) -> Result<(), String> {
        let component = self.component();
        let handled = component.input_handler().map_or(Ok(()), |handler| {
            handler
                .try_handle_input(data)
                .map_err(|error| error.to_string())
        });
        let failed = self.failure.borrow_mut().take();
        failed.map_or(handled, Err)
    }

    /// Replaces or removes the background of text or a box.
    fn background(&self, bg: Option<Paint>, registry: &Registry) -> Result<(), String> {
        let bg = bg.map(|bg| registry.paint(bg, &self.failure)).transpose()?;
        match &self.kind {
            Kind::Text(widget) => widget.set_custom_bg_fn(bg),
            Kind::Box(widget) => widget.set_bg_fn(bg),
            _ => return Err("the widget has no background".to_owned()),
        }
        Ok(())
    }

    /// Adds, removes or clears children of a container or box, replying to the request.
    fn children(&self, request: Request, registry: &Registry) {
        let parent = match &self.kind {
            Kind::Container(widget) => Parent::Container(widget),
            Kind::Box(widget) => Parent::Box(widget),
            Kind::List(list) => match list.rows() {
                Some(rows) => Parent::Container(rows),
                None => return request.reject("the widget has no children"),
            },
            _ => return request.reject("the widget has no children"),
        };
        match request {
            Request::Add(child, reply) => {
                let child = self.child(child, registry);
                let _ = reply.send(child.map(|child| parent.add(child)));
            }
            Request::Remove(key, reply) => {
                let child = self.key(key, registry);
                let _ = reply
                    .send(child.map(|child| child.iter().for_each(|child| parent.remove(child))));
            }
            Request::Clear(reply) => {
                parent.clear();
                let removed = std::mem::take(&mut *self.guests.borrow_mut());
                drop(removed);
                let _ = reply.send(Ok(()));
            }
            request => request.reject("expected a child request"),
        }
    }

    /// The component of a child to add; an authored one is kept under its identity.
    fn child(&self, child: Child, registry: &Registry) -> Result<ComponentHandle, String> {
        match child {
            Child::Widget(widget) => registry.widget(&widget).map(|widget| widget.component()),
            Child::Component(handle) => {
                let id = handle.id();
                let view = crate::guest_component::GuestComponent::new(
                    handle,
                    registry.clone(),
                    self.events.clone(),
                );
                let view: ComponentHandle = Rc::new(view);
                self.guests.borrow_mut().push((id, view.clone()));
                Ok(view)
            }
        }
    }

    /// The component of a child to remove, absent when no authored child has the identity.
    fn key(&self, key: Key, registry: &Registry) -> Result<Option<ComponentHandle>, String> {
        match key {
            Key::Widget(widget) => registry
                .widget(&widget)
                .map(|widget| Some(widget.component())),
            Key::Component(id) => {
                let mut guests = self.guests.borrow_mut();
                let index = guests.iter().position(|(added, _)| *added == id);
                Ok(index.map(|index| guests.remove(index).1))
            }
        }
    }
}

/// A widget that holds children.
enum Parent<'a> {
    /// A container.
    Container(&'a Container),
    /// A box.
    Box(&'a Boxed),
}

impl Parent<'_> {
    /// Appends a child.
    fn add(&self, child: ComponentHandle) {
        match self {
            Self::Container(parent) => parent.add_child(child),
            Self::Box(parent) => parent.add_child(child),
        }
    }
    /// Removes the first occurrence of a child.
    fn remove(&self, child: &ComponentHandle) {
        match self {
            Self::Container(parent) => parent.remove_child(child),
            Self::Box(parent) => parent.remove_child(child),
        }
    }
    /// Removes every child.
    fn clear(&self) {
        match self {
            Self::Container(parent) => parent.clear(),
            Self::Box(parent) => parent.clear(),
        }
    }
}

impl Request {
    /// Returns a catchable failure.
    pub(crate) fn reject(self, error: &str) {
        match self {
            Self::Render(_, reply) => drop(reply.send(Err(error.to_owned()))),
            Self::Invalidate(reply)
            | Self::SetText(_, reply)
            | Self::SetBackground(_, reply)
            | Self::SetLines(_, reply)
            | Self::Add(_, reply)
            | Self::Remove(_, reply)
            | Self::Clear(reply)
            | Self::Input(_, reply) => drop(reply.send(Err(error.to_owned()))),
            Self::List(request) => request.reject(error),
        }
    }
}

/// A count from the wire as the toolkit's count.
fn width(value: u32) -> Result<usize, String> {
    usize::try_from(value).map_err(|error| error.to_string())
}

/// Markdown styles from their operands.
fn markdown_theme(
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
fn base_style(
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

/// Store-safe identity of one host widget.
#[derive(Clone)]
pub(crate) struct WidgetEndpoint(pub(crate) Arc<Lease>);

impl WidgetEndpoint {
    /// Sends one request that makes no call back into this instance.
    pub(crate) fn read<T>(&self, request: impl FnOnce(Reply<T>) -> Request) -> Result<T, String> {
        self.0
            .request(|id, reply| Message::Widgets(WidgetMessage::Request(id, request(reply))))
    }

    /// Sends one request whose native code may call back into this instance.
    pub(crate) fn route<T: Send + 'static>(
        &self,
        request: impl FnOnce(Reply<T>) -> Request,
    ) -> crate::import_route::Route<T> {
        self.0
            .route(|id, reply| Message::Widgets(WidgetMessage::Request(id, request(reply))))
    }
}

/// Store-safe identity of one lent host style.
#[derive(Clone)]
pub(crate) struct StyleEndpoint(pub(crate) Arc<Lease>);

/// Store-safe identity of the lent host highlighting.
#[derive(Clone)]
pub(crate) struct HighlighterEndpoint(pub(crate) Arc<Lease>);

impl StyleEndpoint {
    /// Applies the style.
    pub(crate) fn apply(&self, text: String) -> Result<String, String> {
        self.0.request(|id, reply| {
            Message::Widgets(WidgetMessage::Lent(id, Lent::Style(text, reply)))
        })
    }
}

impl HighlighterEndpoint {
    /// Runs the highlighting.
    pub(crate) fn highlight(
        &self,
        code: String,
        lang: Option<String>,
    ) -> Result<Vec<String>, String> {
        self.0.request(|id, reply| {
            Message::Widgets(WidgetMessage::Lent(id, Lent::Highlight(code, lang, reply)))
        })
    }
}

/// The lent Markdown styles in field order, and the lent highlighting.
pub(crate) type LentTheme = (Vec<StyleEndpoint>, HighlighterEndpoint);

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

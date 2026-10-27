//! Host widgets an extension built, served on the local owner over the toolkit widgets.
use crate::guest_calls::Handle;
use crate::guest_calls::paints::Failure;
use crate::local_resources::{Lease, Message, Registry};
use maestro_extensions::widgets::{
    Box as Boxed, Container, ExtensionWidgets, Markdown, MarkdownOptions, Spacer, Text,
    TextDecoration,
};
use maestro_extensions::{ComponentHandle, EventBus, Local};
use std::{cell::OnceCell, cell::RefCell, rc::Rc, sync::Arc, sync::mpsc::Sender};

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
    /// An editor or custom editor.
    Editor(crate::widget_editors::EditorBuild),
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
    /// A request only an editor takes.
    Editor(crate::widget_editors::EditorRequest),
    /// A request only a frame writer takes; it answers through the import route.
    Writer(
        crate::frame_writer::Request,
        Reply<crate::import_route::Value>,
    ),
    /// Adds a frame writer's authored input listener.
    AddInputListener(
        Handle,
        Reply<crate::local_resources::effects::SubscriptionEndpoint>,
    ),
}

/// A message for the widgets of one extension.
pub(crate) enum WidgetMessage {
    /// Requests on retained overlay capabilities.
    Overlay(crate::overlay_reads::OverlayMessage),
    /// The full redraw count of the original frame writer.
    FullRedraws(u64, Reply<u64>),
    /// Builds the writer widget of a lent frame writer for the extension whose API has this
    /// identity.
    Writer(
        u64,
        crate::local_resources::effects::TuiEndpoint,
        Reply<WidgetEndpoint>,
    ),
    /// Discards pending input of the terminal of the writer widget with this identity.
    Drain(
        u64,
        (Option<f64>, Option<f64>),
        tokio::sync::oneshot::Sender<Result<(), String>>,
        crate::local_resources::Admission,
    ),
    /// Retains the host's live theme independently of UI availability.
    GlobalTheme(Reply<crate::theme_reads::Endpoint>),
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
    /// Admits one suggestion request on a lent completion provider.
    Suggestions(
        u64,
        crate::guest_calls::providers::SuggestionRequest,
        crate::local_resources::Admission,
    ),
    /// One request on a lent completion provider.
    Provider(u64, crate::guest_calls::providers::Lent),
    /// Builds an editor with another extension's editor factory for the extension whose API
    /// has this identity.
    Foreign(u64, crate::widget_foreign::ForeignBuild),
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
    /// Runs a lent text callback.
    Text(String, Reply<()>),
}

impl Lent {
    /// Returns a catchable failure.
    pub(crate) fn reject(self, error: &str) {
        let error = error.to_owned();
        match self {
            Self::Style(_, reply) | Self::Row(_, _, reply) => drop(reply.send(Err(error))),
            Self::Highlight(_, _, reply) => drop(reply.send(Err(error))),
            Self::Done(_, reply) | Self::Text(_, reply) => drop(reply.send(Err(error))),
        }
    }
}

/// The toolkit widget behind one host widget.
pub(crate) enum Kind {
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
    /// An editor or custom editor.
    Editor(crate::widget_editors::EditorKind),
    /// A lent frame writer and the component it renders as.
    Writer(Rc<crate::frame_writer::Writer>, ComponentHandle),
}

/// One host widget, the failure its authored functions report and its authored children.
pub(crate) struct Widget {
    /// The toolkit widget.
    pub(crate) kind: Kind,
    /// The first failure of an authored function since the last rendering.
    pub(crate) failure: Failure,
    /// The extension's bus, whose frames enter authored children.
    pub(crate) events: EventBus<Local>,
    /// Authored children by the identity each was added under, in insertion order.
    pub(crate) guests: RefCell<Vec<(u32, ComponentHandle)>>,
    /// The widget as another component's child, built once so removal finds it.
    pub(crate) nested: OnceCell<ComponentHandle>,
}

impl Widget {
    /// The application's custom editor, when the widget is one.
    pub(crate) fn custom_editor(
        &self,
    ) -> Option<std::rc::Rc<dyn maestro_extensions::types::CustomEditorComponent>> {
        match &self.kind {
            Kind::Editor(editor) => editor.custom(),
            _ => None,
        }
    }

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
            Kind::Editor(editor) => editor.component(),
            Kind::Writer(_, component) => component.clone(),
        }
    }

    /// Keeps an editor whose authored callbacks report to `failure` and enter `events`.
    pub(crate) fn editor(
        editor: crate::widget_editors::EditorKind,
        failure: Failure,
        events: EventBus<Local>,
    ) -> Self {
        Self {
            kind: Kind::Editor(editor),
            failure,
            events,
            guests: RefCell::default(),
            nested: OnceCell::new(),
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
        let binding = crate::widget_lists::Binding {
            registry,
            events: &events,
            failure: &failure,
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
            Build::List(list) => Kind::List(crate::widget_lists::ListKind::build(
                list, &binding, widgets,
            )?),
            Build::Editor(editor) => Kind::Editor(crate::widget_editors::EditorKind::build(
                editor, &binding, widgets,
            )?),
            Build::Markdown(text, padding, styles, base) => {
                let theme = crate::widget_markdown::markdown_theme(registry, styles, &failure)?;
                let options = MarkdownOptions {
                    clean_content: false,
                    padding_x: width(padding.0)?,
                    padding_y: width(padding.1)?,
                    default_text_style: base
                        .map(|base| crate::widget_markdown::base_style(registry, base, &failure))
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
            nested: OnceCell::new(),
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
                Kind::List(list) => list.answer(request, &self.binding(registry)),
                Kind::Editor(editor) => editor.answer_list(request, &self.binding(registry)),
                _ => request.reject("the widget is no list"),
            },
            Request::Editor(request) => match &self.kind {
                Kind::Editor(editor) => editor.answer(request, &self.binding(registry)),
                _ => request.reject("the widget is no editor"),
            },
            Request::Writer(request, reply) => {
                let _ = reply.send(self.writer_request(request, registry));
            }
            Request::AddInputListener(handle, reply) => {
                let _ = reply.send(self.add_input_listener(handle, registry));
            }
            request => self.children(request, registry),
        }
    }

    /// What the widget's authored callbacks are bound to.
    pub(crate) fn binding<'a>(
        &'a self,
        registry: &'a Registry,
    ) -> crate::widget_lists::Binding<'a> {
        crate::widget_lists::Binding {
            registry,
            events: &self.events,
            failure: &self.failure,
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
            Kind::Writer(writer, _) => Parent::Writer(&writer.tui),
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
            Child::Widget(widget) => registry.widget(&widget).map(|widget| widget.nested()),
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
            Key::Widget(widget) => registry.widget(&widget).map(|widget| Some(widget.nested())),
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
    /// A frame writer.
    Writer(&'a maestro_extensions::TUI),
}

impl Parent<'_> {
    /// Appends a child.
    fn add(&self, child: ComponentHandle) {
        match self {
            Self::Container(parent) => parent.add_child(child),
            Self::Box(parent) => parent.add_child(child),
            Self::Writer(parent) => parent.add_child(child),
        }
    }
    /// Removes the first occurrence of a child.
    fn remove(&self, child: &ComponentHandle) {
        match self {
            Self::Container(parent) => parent.remove_child(child),
            Self::Box(parent) => parent.remove_child(child),
            Self::Writer(parent) => parent.remove_child(child),
        }
    }
    /// Removes every child.
    fn clear(&self) {
        match self {
            Self::Container(parent) => parent.clear(),
            Self::Box(parent) => parent.clear(),
            Self::Writer(parent) => parent.clear(),
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
            Self::Editor(request) => request.reject(error),
            Self::Writer(_, reply) => drop(reply.send(Err(error.to_owned()))),
            Self::AddInputListener(_, reply) => drop(reply.send(Err(error.to_owned()))),
        }
    }
}

/// A count from the wire as the toolkit's count.
fn width(value: u32) -> Result<usize, String> {
    usize::try_from(value).map_err(|error| error.to_string())
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
    pub(crate) fn route<T: Into<crate::import_route::Value> + Send + 'static>(
        &self,
        request: impl FnOnce(Reply<T>) -> Request,
    ) -> crate::import_route::Route {
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

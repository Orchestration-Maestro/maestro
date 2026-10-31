//! Host selection lists, settings lists and loaders, served on the local owner.
use crate::guest_calls::Handle;
use crate::guest_calls::paints::Failure;
use crate::guest_calls::widget_calls::{Answered, Event, Item, Truncate};
use crate::local_resources::{Lease, Message, Registry, SignalEndpoint, effects::TuiEndpoint};
use crate::widget_reads::{Lent, Paint, Reply, WidgetMessage};
use maestro_extensions::widgets::{
    Container, ExtensionLoader, ExtensionWidgets, SelectItem, SelectList, SelectListLayoutOptions,
    SelectListTheme, SelectListTruncatePrimaryContext, SettingItem, SettingsList,
    SettingsListOptions, SettingsListTheme,
};
use maestro_extensions::{ComponentHandle, EventBus, Local};
use std::{rc::Rc, sync::Arc};

/// A toolkit settings row style.
pub(crate) type RowStyleFn = dyn Fn(&str, bool) -> String;
/// A toolkit settings submenu: the current value and the closer to a component.
type SubmenuFn = dyn Fn(String, Rc<dyn Fn(Option<String>)>) -> ComponentHandle;

/// Store-safe identity of the closer of one open settings submenu.
#[derive(Clone)]
pub(crate) struct SubmenuDoneEndpoint(pub(crate) Arc<Lease>);

impl SubmenuDoneEndpoint {
    /// Closes the submenu; the list's change callback may call back into this instance.
    pub(crate) fn done(&self, value: Option<String>) -> crate::import_route::Route {
        self.0
            .route(|id, reply| Message::Widgets(WidgetMessage::Lent(id, Lent::Done(value, reply))))
    }
}

/// Store-safe identity of one lent settings row style.
#[derive(Clone)]
pub(crate) struct RowStyleEndpoint(pub(crate) Arc<Lease>);

impl RowStyleEndpoint {
    /// Applies the row style.
    pub(crate) fn apply(&self, text: String, selected: bool) -> Result<String, String> {
        self.0.request(|id, reply| {
            Message::Widgets(WidgetMessage::Lent(id, Lent::Row(text, selected, reply)))
        })
    }
}

/// A settings row style operand: a lent host row style, or an authored one.
pub(crate) enum RowPaint {
    /// A lent host row style.
    Host(RowStyleEndpoint),
    /// An authored row style.
    Authored(Handle),
}

/// One setting as the guest sent it.
pub(crate) struct SettingOperand {
    /// Identity.
    pub(crate) id: String,
    /// Label.
    pub(crate) label: String,
    /// Description.
    pub(crate) description: Option<String>,
    /// Current value.
    pub(crate) value: String,
    /// Values activation cycles through.
    pub(crate) values: Option<Vec<String>>,
    /// The authored submenu.
    pub(crate) submenu: Option<Handle>,
}

/// Settings list styles as the guest sent them.
pub(crate) struct SettingsPaints {
    /// Labels.
    pub(crate) label: RowPaint,
    /// Values.
    pub(crate) value: RowPaint,
    /// Descriptions.
    pub(crate) description: Paint,
    /// The selected row's prefix.
    pub(crate) cursor: String,
    /// The hint row.
    pub(crate) hint: Paint,
}

/// What a new list or loader is built from.
pub(crate) enum ListBuild {
    /// A selection list: choices, visible rows, styles in field order and layout.
    Select(
        Vec<Item>,
        u32,
        Vec<Paint>,
        (Option<i64>, Option<i64>, Option<Handle>),
    ),
    /// A settings list: settings, visible rows, styles, change and cancel callbacks, search.
    Settings(
        (Vec<SettingOperand>, u32),
        SettingsPaints,
        (Handle, Handle),
        bool,
    ),
    /// A loader on a lent terminal and theme, with its message and cancellability.
    Loader(
        TuiEndpoint,
        crate::theme_reads::Endpoint,
        String,
        Option<bool>,
    ),
}

/// The handler slot a request replaces.
#[derive(Clone, Copy)]
pub(crate) enum Slot {
    /// Confirmation.
    Select,
    /// Cancellation.
    Cancel,
    /// Selection movement.
    SelectionChange,
    /// A loader's abort.
    Abort,
    /// An editor's submission.
    Submit,
    /// An editor's change.
    Change,
    /// A custom editor's interrupt.
    Escape,
    /// A custom editor's empty-editor exit.
    CtrlD,
    /// A custom editor's image paste.
    PasteImage,
    /// A custom editor's input interceptor.
    ExtensionShortcut,
}

/// One request on a list or loader.
pub(crate) enum ListRequest {
    /// Filters a selection list.
    SetFilter(String, Reply<()>),
    /// Selects a row of a selection list.
    SetSelectedIndex(i64, Reply<()>),
    /// Reads the selected choice of a selection list.
    Selected(Reply<Option<Item>>),
    /// Replaces or removes a handler.
    SetHandler(Slot, Option<Handle>, Reply<()>),
    /// Replaces a setting's value.
    UpdateValue(String, String, Reply<()>),
    /// Reads a loader's signal.
    Signal(Reply<SignalEndpoint>),
    /// Stops a loader's animation.
    Dispose(Reply<()>),
}

impl ListRequest {
    /// Returns a catchable failure.
    pub(crate) fn reject(self, error: &str) {
        let error = error.to_owned();
        match self {
            Self::Selected(reply) => drop(reply.send(Err(error))),
            Self::Signal(reply) => drop(reply.send(Err(error))),
            Self::SetFilter(_, reply)
            | Self::SetSelectedIndex(_, reply)
            | Self::SetHandler(_, _, reply)
            | Self::UpdateValue(_, _, reply)
            | Self::Dispose(reply) => drop(reply.send(Err(error))),
        }
    }
}

/// The toolkit or application widget behind one list or loader.
pub(crate) enum ListKind {
    /// A selection list.
    Select(Rc<SelectList>),
    /// A settings list.
    Settings(Rc<SettingsList>),
    /// A bordered loader.
    Loader(Rc<dyn ExtensionLoader>),
}

/// What authored list callbacks are bound to.
pub(crate) struct Binding<'a> {
    /// The capability registry.
    pub(crate) registry: &'a Registry,
    /// The extension's bus.
    pub(crate) events: &'a EventBus<Local>,
    /// Where an authored callback's failure goes.
    pub(crate) failure: &'a Failure,
}

impl Binding<'_> {
    /// Calls an authored callback, keeping its failure.
    pub(crate) fn call(&self, handle: &Handle, event: Event) -> Option<Answered> {
        match handle.widget(self.registry, self.events, event) {
            Ok(answer) => Some(answer),
            Err(error) => {
                self.failure.borrow_mut().get_or_insert(error);
                None
            }
        }
    }

    /// An authored choice handler.
    fn item(&self, handle: Handle, moved: bool) -> Rc<dyn Fn(Rc<SelectItem>)> {
        let (registry, events, failure) = self.owned();
        Rc::new(move |item: Rc<SelectItem>| {
            let binding = Binding {
                registry: &registry,
                events: &events,
                failure: &failure,
            };
            let item = wire_item(&item);
            let event = if moved {
                Event::SelectionChange(item)
            } else {
                Event::Select(item)
            };
            binding.call(&handle, event);
        })
    }

    /// An authored notice handler.
    pub(crate) fn notice(&self, handle: Handle, event: fn() -> Event) -> Rc<dyn Fn()> {
        let (registry, events, failure) = self.owned();
        Rc::new(move || {
            let binding = Binding {
                registry: &registry,
                events: &events,
                failure: &failure,
            };
            binding.call(&handle, event());
        })
    }

    /// Owned copies of the binding, for a retained callback.
    pub(crate) fn owned(&self) -> (Registry, EventBus<Local>, Failure) {
        (
            self.registry.clone(),
            self.events.clone(),
            self.failure.clone(),
        )
    }
}

/// The wire form of a toolkit choice.
fn wire_item(item: &SelectItem) -> Item {
    Item::new(&item.value, &item.label, item.description.as_ref())
}

/// A toolkit choice from its wire form.
fn toolkit_item(item: Item) -> SelectItem {
    SelectItem {
        value: item.value,
        label: item.label,
        description: item.description,
    }
}

impl ListKind {
    /// The list or loader as a toolkit component.
    pub(crate) fn component(&self) -> ComponentHandle {
        match self {
            Self::Select(list) => list.clone(),
            Self::Settings(list) => list.clone(),
            Self::Loader(loader) => loader.clone(),
        }
    }

    /// The rows of a loader, which an extension may extend.
    pub(crate) fn rows(&self) -> Option<&Container> {
        match self {
            Self::Loader(loader) => Some(loader.rows()),
            _ => None,
        }
    }

    /// Builds the list or loader `build` names.
    pub(crate) fn build(
        build: ListBuild,
        binding: &Binding<'_>,
        widgets: &dyn ExtensionWidgets,
    ) -> Result<Self, String> {
        match build {
            ListBuild::Select(items, max_visible, styles, layout) => {
                select_list(binding, (items, max_visible), styles, layout).map(Self::Select)
            }
            ListBuild::Settings(items, paints, callbacks, search) => {
                settings_list(binding, items, paints, (callbacks, search)).map(Self::Settings)
            }
            ListBuild::Loader(tui, theme, message, cancellable) => {
                let tui = binding.registry.terminal(&tui)?;
                let theme = binding.registry.live_theme(&theme)?;
                let theme = theme.get().map_err(|error| error.to_string())?;
                let cancellable = cancellable.unwrap_or(true);
                widgets
                    .bordered_loader(&tui, theme, message, cancellable)
                    .map(Self::Loader)
            }
        }
    }

    /// Answers one request.
    pub(crate) fn answer(&self, request: ListRequest, binding: &Binding<'_>) {
        match (self, request) {
            (Self::Select(list), ListRequest::SetFilter(filter, reply)) => {
                list.set_filter(&filter);
                let _ = reply.send(Ok(()));
            }
            (Self::Select(list), ListRequest::SetSelectedIndex(index, reply)) => {
                let index =
                    isize::try_from(index).unwrap_or(if index < 0 { 0 } else { isize::MAX });
                list.set_selected_index(index);
                let _ = reply.send(Ok(()));
            }
            (Self::Select(list), ListRequest::Selected(reply)) => {
                let _ = reply.send(Ok(list.get_selected_item().map(|item| wire_item(&item))));
            }
            (Self::Settings(list), ListRequest::UpdateValue(id, value, reply)) => {
                list.update_value(&id, value);
                let _ = reply.send(Ok(()));
            }
            (Self::Loader(loader), ListRequest::Signal(reply)) => {
                let _ = reply.send(binding.registry.signal(loader.signal()));
            }
            (Self::Loader(loader), ListRequest::Dispose(reply)) => {
                loader.dispose();
                let _ = reply.send(Ok(()));
            }
            (list, ListRequest::SetHandler(slot, handler, reply)) => {
                let _ = reply.send(list.set_handler(slot, handler, binding));
            }
            (_, request) => request.reject("the widget does not take this request"),
        }
    }

    /// Replaces or removes one handler.
    fn set_handler(
        &self,
        slot: Slot,
        handler: Option<Handle>,
        binding: &Binding<'_>,
    ) -> Result<(), String> {
        match (self, slot) {
            (Self::Select(list), Slot::Select) => {
                list.set_on_select(handler.map(|handle| binding.item(handle, false)));
            }
            (Self::Select(list), Slot::SelectionChange) => {
                list.set_on_selection_change(handler.map(|handle| binding.item(handle, true)));
            }
            (Self::Select(list), Slot::Cancel) => {
                list.set_on_cancel(handler.map(|handle| binding.notice(handle, || Event::Cancel)));
            }
            (Self::Loader(loader), Slot::Abort) => {
                loader.set_on_abort(handler.map(|handle| binding.notice(handle, || Event::Abort)));
            }
            _ => return Err("the widget has no such handler".to_owned()),
        }
        Ok(())
    }
}

/// Builds a selection list with its styles and layout.
fn select_list(
    binding: &Binding<'_>,
    (items, max_visible): (Vec<Item>, u32),
    styles: Vec<Paint>,
    (min, max, truncate): (Option<i64>, Option<i64>, Option<Handle>),
) -> Result<Rc<SelectList>, String> {
    let mut styles = styles
        .into_iter()
        .map(|paint| binding.registry.paint(paint, binding.failure));
    let mut next = || {
        styles
            .next()
            .unwrap_or_else(|| Err("a selection list style is missing".to_owned()))
    };
    let theme = SelectListTheme {
        selected_prefix: next()?,
        selected_text: next()?,
        description: next()?,
        scroll_info: next()?,
        no_match: next()?,
    };
    let width = |value: Option<i64>| value.map(isize::try_from).transpose();
    let layout = SelectListLayoutOptions {
        min_primary_column_width: width(min).map_err(|error| error.to_string())?,
        max_primary_column_width: width(max).map_err(|error| error.to_string())?,
        truncate_primary: truncate.map(|handle| truncation(binding, handle)),
    };
    let items = items.into_iter().map(toolkit_item).collect();
    let max_visible = usize::try_from(max_visible).map_err(|error| error.to_string())?;
    Ok(Rc::new(SelectList::new(items, max_visible, theme, layout)))
}

/// An authored primary-column truncation; a failure keeps the text.
fn truncation(
    binding: &Binding<'_>,
    handle: Handle,
) -> Rc<dyn for<'a> Fn(SelectListTruncatePrimaryContext<'a>) -> String> {
    let (registry, events, failure) = binding.owned();
    Rc::new(move |context: SelectListTruncatePrimaryContext<'_>| {
        let binding = Binding {
            registry: &registry,
            events: &events,
            failure: &failure,
        };
        let event = Event::Truncate(Truncate {
            text: context.text.to_owned(),
            max_width: i64::try_from(context.max_width).unwrap_or(i64::MAX),
            column_width: i64::try_from(context.column_width).unwrap_or(i64::MAX),
            item: wire_item(&context.item),
            is_selected: context.is_selected,
        });
        match binding.call(&handle, event) {
            Some(Answered::Text(text)) => text,
            _ => context.text.to_owned(),
        }
    })
}

/// Builds a settings list with its styles, submenus and callbacks.
fn settings_list(
    binding: &Binding<'_>,
    (items, max_visible): (Vec<SettingOperand>, u32),
    paints: SettingsPaints,
    ((change, cancel), search): ((Handle, Handle), bool),
) -> Result<Rc<SettingsList>, String> {
    let theme = SettingsListTheme {
        label: row_style(binding, paints.label)?,
        value: row_style(binding, paints.value)?,
        description: binding
            .registry
            .paint(paints.description, binding.failure)?,
        cursor: paints.cursor,
        hint: binding.registry.paint(paints.hint, binding.failure)?,
    };
    let items = items
        .into_iter()
        .map(|item| SettingItem {
            id: item.id,
            label: item.label,
            description: item.description,
            current_value: item.value,
            values: item.values,
            submenu: item.submenu.map(|handle| submenu(binding, handle)),
        })
        .collect();
    let (registry, events, failure) = binding.owned();
    let on_change = Rc::new(move |id: &str, value: &str| {
        let binding = Binding {
            registry: &registry,
            events: &events,
            failure: &failure,
        };
        binding.call(&change, Event::Change(id.to_owned(), value.to_owned()));
        true
    });
    let on_cancel = binding.notice(cancel, || Event::Cancel);
    let max_visible = usize::try_from(max_visible).map_err(|error| error.to_string())?;
    let options = SettingsListOptions {
        enable_search: search,
    };
    Ok(Rc::new(SettingsList::new(
        items,
        max_visible,
        theme,
        (on_change, on_cancel),
        options,
    )))
}

/// The toolkit row style a row style operand names.
fn row_style(binding: &Binding<'_>, paint: RowPaint) -> Result<Rc<RowStyleFn>, String> {
    match paint {
        RowPaint::Host(style) => binding.registry.row_style(&style),
        RowPaint::Authored(handle) => {
            let (registry, events, failure) = binding.owned();
            Ok(Rc::new(move |text: &str, selected: bool| {
                let binding = Binding {
                    registry: &registry,
                    events: &events,
                    failure: &failure,
                };
                match binding.call(&handle, Event::Row(text.to_owned(), selected)) {
                    Some(Answered::Text(text)) => text,
                    _ => text.to_owned(),
                }
            }))
        }
    }
}

/// An authored submenu; a failure opens an empty submenu.
fn submenu(binding: &Binding<'_>, handle: Handle) -> Rc<SubmenuFn> {
    let (registry, events, failure) = binding.owned();
    Rc::new(move |value: String, done: Rc<dyn Fn(Option<String>)>| {
        let binding = Binding {
            registry: &registry,
            events: &events,
            failure: &failure,
        };
        let opened = registry
            .submenu_done(done)
            .map(|done| binding.call(&handle, Event::Submenu(value, done)));
        match opened {
            Ok(Some(Answered::Component(component))) => {
                Rc::new(crate::guest_component::GuestComponent::new(
                    component,
                    registry.clone(),
                    events.clone(),
                ))
            }
            Ok(_) => Rc::new(Container::new()),
            Err(error) => {
                failure.borrow_mut().get_or_insert(error);
                Rc::new(Container::new())
            }
        }
    })
}

/// The lent selection list styles in field order.
pub(crate) fn select_roles(
    theme: &Rc<SelectListTheme>,
) -> Vec<maestro_extensions::widgets::BorderColor> {
    vec![
        theme.selected_prefix.clone(),
        theme.selected_text.clone(),
        theme.description.clone(),
        theme.scroll_info.clone(),
        theme.no_match.clone(),
    ]
}

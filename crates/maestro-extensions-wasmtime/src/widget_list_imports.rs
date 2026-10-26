//! Canonical list and loader imports delegate to the host's widgets.
use crate::{
    callback_imports::{CallbackView, retain},
    guest_calls::{Callback, widget_calls::Item},
    import_route::push,
    local_resources::{Message, SignalEndpoint, effects::TuiEndpoint},
    widget_imports::{StyleSource, build, paint, widget},
    widget_lists::{
        ListBuild, ListRequest, RowPaint, RowStyleEndpoint, SettingOperand, SettingsPaints, Slot,
        SubmenuDoneEndpoint,
    },
    widget_reads::{Build, Paint, Request, StyleEndpoint, WidgetEndpoint, WidgetMessage},
};
use wasmtime::{
    StoreContextMut,
    component::{ComponentType, Lift, LinkerInstance, Lower, Resource},
};

/// Selection list styles as the guest sends them.
#[derive(ComponentType, Lift)]
#[component(record)]
pub(crate) struct SelectListStyles {
    /// The selected row's prefix.
    #[component(name = "selected-prefix")]
    selected_prefix: StyleSource,
    /// The selected row's text.
    #[component(name = "selected-text")]
    selected_text: StyleSource,
    /// Descriptions.
    description: StyleSource,
    /// The scroll position.
    #[component(name = "scroll-info")]
    scroll_info: StyleSource,
    /// The no-match notice.
    #[component(name = "no-match")]
    no_match: StyleSource,
}

impl SelectListStyles {
    /// The paints of the styles in field order, retaining authored ones.
    pub(crate) fn paints<T: CallbackView>(&self, store: &mut T) -> wasmtime::Result<Vec<Paint>> {
        [
            &self.selected_prefix,
            &self.selected_text,
            &self.description,
            &self.scroll_info,
            &self.no_match,
        ]
        .into_iter()
        .map(|style| paint(store, style))
        .collect()
    }
}

/// Primary column layout as the guest sends it.
#[derive(ComponentType, Lift)]
#[component(record)]
struct SelectLayout {
    /// The narrowest primary column.
    #[component(name = "min-primary-column-width")]
    min_primary_column_width: Option<i64>,
    /// The widest primary column.
    #[component(name = "max-primary-column-width")]
    max_primary_column_width: Option<i64>,
    /// The authored truncation.
    #[component(name = "truncate-primary")]
    truncate_primary: Option<Resource<Callback>>,
}

/// A settings row styling operand as the guest sends it.
#[derive(ComponentType, Lift)]
#[component(variant)]
enum RowStyleSource {
    /// A lent host row style.
    #[component(name = "host")]
    Host(Resource<RowStyleEndpoint>),
    /// An authored row style.
    #[component(name = "authored")]
    Authored(Resource<Callback>),
}

/// Settings list styles as the guest sends them.
#[derive(ComponentType, Lift)]
#[component(record)]
struct SettingsListStyles {
    /// Labels.
    label: RowStyleSource,
    /// Values.
    value: RowStyleSource,
    /// Descriptions.
    description: StyleSource,
    /// The selected row's prefix.
    cursor: String,
    /// The hint row.
    hint: StyleSource,
}

/// One setting as the guest sends it.
#[derive(ComponentType, Lift)]
#[component(record)]
struct SettingItem {
    /// Identity.
    id: String,
    /// Label.
    label: String,
    /// Description.
    description: Option<String>,
    /// Current value.
    #[component(name = "current-value")]
    current_value: String,
    /// Values activation cycles through.
    values: Option<Vec<String>>,
    /// The authored submenu.
    submenu: Option<Resource<Callback>>,
}

/// The handler slot a request replaces.
#[derive(Clone, Copy, ComponentType, Lift)]
#[component(enum)]
#[repr(u8)]
#[expect(
    dead_code,
    reason = "constructed by the Wasmtime Lift macro from wire values; host code only matches them"
)]
enum HandlerSlot {
    /// Confirmation.
    #[component(name = "select")]
    Select,
    /// Cancellation.
    #[component(name = "cancel")]
    Cancel,
    /// Selection movement.
    #[component(name = "selection-change")]
    SelectionChange,
    /// A loader's abort.
    #[component(name = "abort")]
    Abort,
    /// An editor's submission.
    #[component(name = "submit")]
    Submit,
    /// An editor's change.
    #[component(name = "change")]
    Change,
    /// A custom editor's interrupt.
    #[component(name = "escape")]
    Escape,
    /// A custom editor's empty-editor exit.
    #[component(name = "ctrl-d")]
    CtrlD,
    /// A custom editor's image paste.
    #[component(name = "paste-image")]
    PasteImage,
    /// A custom editor's input interceptor.
    #[component(name = "extension-shortcut")]
    ExtensionShortcut,
}

impl From<HandlerSlot> for Slot {
    fn from(slot: HandlerSlot) -> Self {
        match slot {
            HandlerSlot::Select => Self::Select,
            HandlerSlot::Cancel => Self::Cancel,
            HandlerSlot::SelectionChange => Self::SelectionChange,
            HandlerSlot::Abort => Self::Abort,
            HandlerSlot::Submit => Self::Submit,
            HandlerSlot::Change => Self::Change,
            HandlerSlot::Escape => Self::Escape,
            HandlerSlot::CtrlD => Self::CtrlD,
            HandlerSlot::PasteImage => Self::PasteImage,
            HandlerSlot::ExtensionShortcut => Self::ExtensionShortcut,
        }
    }
}

/// The lent selection list styles.
#[derive(ComponentType, Lower)]
#[component(record)]
struct SelectListTheme {
    /// The selected row's prefix.
    #[component(name = "selected-prefix")]
    selected_prefix: Resource<StyleEndpoint>,
    /// The selected row's text.
    #[component(name = "selected-text")]
    selected_text: Resource<StyleEndpoint>,
    /// Descriptions.
    description: Resource<StyleEndpoint>,
    /// The scroll position.
    #[component(name = "scroll-info")]
    scroll_info: Resource<StyleEndpoint>,
    /// The no-match notice.
    #[component(name = "no-match")]
    no_match: Resource<StyleEndpoint>,
}

/// The lent settings list styles.
#[derive(ComponentType, Lower)]
#[component(record)]
struct SettingsListTheme {
    /// Labels.
    label: Resource<RowStyleEndpoint>,
    /// Values.
    value: Resource<RowStyleEndpoint>,
    /// Descriptions.
    description: Resource<StyleEndpoint>,
    /// The selected row's prefix.
    cursor: String,
    /// The hint row.
    hint: Resource<StyleEndpoint>,
}

/// The row paint a row styling operand names, retaining an authored style's identity.
fn row_paint<T: CallbackView>(store: &mut T, style: &RowStyleSource) -> wasmtime::Result<RowPaint> {
    Ok(match style {
        RowStyleSource::Host(style) => RowPaint::Host(store.ctx().table.get(style)?.clone()),
        RowStyleSource::Authored(callback) => RowPaint::Authored(retain(store, callback)?),
    })
}

/// Installs the list and loader constructors, methods and lent styles.
pub(crate) fn add_to_linker<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    crate::context_imports::add_resource::<T, RowStyleEndpoint>(host, "row-style")?;
    crate::context_imports::add_resource::<T, SubmenuDoneEndpoint>(host, "submenu-done")?;
    constructors(host)?;
    themes(host)?;
    methods(host)?;
    reads(host)?;
    lent(host)
}

/// Installs the selection list, settings list and loader constructors.
fn constructors<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    host.func_wrap(
        "new-select-list",
        |mut store: StoreContextMut<'_, T>,
         (items, max_visible, theme, layout): (Vec<Item>, u32, SelectListStyles, SelectLayout)| {
            let store = store.data_mut();
            let styles = theme.paints(store)?;
            let truncate = layout
                .truncate_primary
                .as_ref()
                .map(|callback| retain(store, callback))
                .transpose()?;
            let widths = (layout.min_primary_column_width, layout.max_primary_column_width);
            let list = ListBuild::Select(items, max_visible, styles, (widths.0, widths.1, truncate));
            Ok((build(store, Build::List(list))?,))
        },
    )?;
    host.func_wrap(
        "new-settings-list",
        |mut store: StoreContextMut<'_, T>,
         (items, max_visible, theme, change, cancel, search): (
            Vec<SettingItem>,
            u32,
            SettingsListStyles,
            Resource<Callback>,
            Resource<Callback>,
            bool,
        )| {
            let store = store.data_mut();
            let list = settings(
                store,
                (items, max_visible),
                &theme,
                (&change, &cancel),
                search,
            )?;
            Ok((build(store, Build::List(list))?,))
        },
    )?;
    loader(host)
}

/// Installs the loader constructor.
fn loader<T: CallbackView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    host.func_wrap(
        "new-bordered-loader",
        |mut store: StoreContextMut<'_, T>,
         (tui, theme, message, cancellable): (
            Resource<TuiEndpoint>,
            Resource<crate::theme_reads::Endpoint>,
            String,
            Option<bool>,
        )| {
            let store = store.data_mut();
            let tui = store.ctx().table.get(&tui)?.clone();
            let theme = store.ctx().table.get(&theme)?.clone();
            let loader = ListBuild::Loader(tui, theme, message, cancellable);
            Ok((build(store, Build::List(loader))?,))
        },
    )
}

/// A settings list's build operands, retaining every authored callback.
fn settings<T: CallbackView>(
    store: &mut T,
    (items, max_visible): (Vec<SettingItem>, u32),
    theme: &SettingsListStyles,
    (change, cancel): (&Resource<Callback>, &Resource<Callback>),
    search: bool,
) -> wasmtime::Result<ListBuild> {
    let mut settings = Vec::new();
    for item in items {
        let submenu = item
            .submenu
            .as_ref()
            .map(|submenu| retain(store, submenu))
            .transpose()?;
        settings.push(SettingOperand {
            id: item.id,
            label: item.label,
            description: item.description,
            value: item.current_value,
            values: item.values,
            submenu,
        });
    }
    let paints = SettingsPaints {
        label: row_paint(store, &theme.label)?,
        value: row_paint(store, &theme.value)?,
        description: paint(store, &theme.description)?,
        cursor: theme.cursor.clone(),
        hint: paint(store, &theme.hint)?,
    };
    let callbacks = (retain(store, change)?, retain(store, cancel)?);
    Ok(ListBuild::Settings(
        (settings, max_visible),
        paints,
        callbacks,
        search,
    ))
}

/// Installs the lent selection and settings list styles.
fn themes<T: CallbackView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    host.func_wrap(
        "get-select-list-theme",
        |mut store: StoreContextMut<'_, T>, (): ()| {
            let lent = store
                .data()
                .api()
                .0
                .request(|_, reply| Message::Widgets(WidgetMessage::LendSelect(reply)));
            let lent = match lent {
                Ok(styles) => Ok(lend_select(store.data_mut(), styles)?),
                Err(error) => Err(error),
            };
            Ok((lent,))
        },
    )?;
    host.func_wrap(
        "get-settings-list-theme",
        |mut store: StoreContextMut<'_, T>, (): ()| {
            let lent = store
                .data()
                .api()
                .0
                .request(|_, reply| Message::Widgets(WidgetMessage::LendSettings(reply)));
            let lent = match lent {
                Ok((label, value, description, cursor, hint)) => {
                    let table = &mut *store.data_mut().ctx().table;
                    Ok(SettingsListTheme {
                        label: table.push(label)?,
                        value: table.push(value)?,
                        description: table.push(description)?,
                        cursor,
                        hint: table.push(hint)?,
                    })
                }
                Err(error) => Err(error),
            };
            Ok((lent,))
        },
    )
}

/// Installs input and the list and loader mutations.
fn methods<T: CallbackView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    host.func_wrap(
        "[method]widget.handle-input",
        |mut store: StoreContextMut<'_, T>, (this, data): (Resource<WidgetEndpoint>, String)| {
            let route: crate::import_route::Route =
                widget(store.data_mut(), &this)?.route(|reply| Request::Input(data, reply));
            Ok((push(store.data_mut(), route)?,))
        },
    )?;
    host.func_wrap(
        "[method]widget.set-filter",
        |mut store: StoreContextMut<'_, T>, (this, filter): (Resource<WidgetEndpoint>, String)| {
            let request = |reply| Request::List(ListRequest::SetFilter(filter, reply));
            Ok((widget(store.data_mut(), &this)?.read(request),))
        },
    )?;
    host.func_wrap(
        "[method]widget.set-selected-index",
        |mut store: StoreContextMut<'_, T>, (this, index): (Resource<WidgetEndpoint>, i64)| {
            let request = |reply| Request::List(ListRequest::SetSelectedIndex(index, reply));
            Ok((widget(store.data_mut(), &this)?.read(request),))
        },
    )?;
    host.func_wrap(
        "[method]widget.set-handler",
        |mut store: StoreContextMut<'_, T>,
         (this, slot, handler): (
            Resource<WidgetEndpoint>,
            HandlerSlot,
            Option<Resource<Callback>>,
        )| {
            let handler = handler
                .as_ref()
                .map(|handler| retain(store.data_mut(), handler))
                .transpose()?;
            let slot = Slot::from(slot);
            let request = |reply| Request::List(ListRequest::SetHandler(slot, handler, reply));
            Ok((widget(store.data_mut(), &this)?.read(request),))
        },
    )?;
    host.func_wrap(
        "[method]widget.update-value",
        |mut store: StoreContextMut<'_, T>,
         (this, id, value): (Resource<WidgetEndpoint>, String, String)| {
            let request = |reply| Request::List(ListRequest::UpdateValue(id, value, reply));
            Ok((widget(store.data_mut(), &this)?.read(request),))
        },
    )?;
    host.func_wrap(
        "[method]widget.dispose",
        |mut store: StoreContextMut<'_, T>, (this,): (Resource<WidgetEndpoint>,)| {
            let request = |reply| Request::List(ListRequest::Dispose(reply));
            Ok((widget(store.data_mut(), &this)?.read(request),))
        },
    )
}

/// Installs the reads of the selected choice and of a loader's signal.
fn reads<T: CallbackView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    host.func_wrap(
        "[method]widget.get-selected-item",
        |mut store: StoreContextMut<'_, T>, (this,): (Resource<WidgetEndpoint>,)| {
            let request = |reply| Request::List(ListRequest::Selected(reply));
            Ok((widget(store.data_mut(), &this)?.read(request),))
        },
    )?;
    host.func_wrap(
        "[method]widget.signal",
        |mut store: StoreContextMut<'_, T>, (this,): (Resource<WidgetEndpoint>,)| {
            let request = |reply| Request::List(ListRequest::Signal(reply));
            let signal = widget(store.data_mut(), &this)?.read(request);
            let signal = match signal {
                Ok(signal) => Ok(store
                    .data_mut()
                    .ctx()
                    .table
                    .push::<SignalEndpoint>(signal)?),
                Err(error) => Err(error),
            };
            Ok((signal,))
        },
    )
}

/// Installs the methods of the lent row styles and submenu closers.
fn lent<T: CallbackView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    host.func_wrap(
        "[method]row-style.apply",
        |mut store: StoreContextMut<'_, T>,
         (this, text, selected): (Resource<RowStyleEndpoint>, String, bool)| {
            let style = store.data_mut().ctx().table.get(&this)?.clone();
            Ok((style.apply(text, selected),))
        },
    )?;
    host.func_wrap(
        "[method]submenu-done.done",
        |mut store: StoreContextMut<'_, T>,
         (this, value): (Resource<SubmenuDoneEndpoint>, Option<String>)| {
            let done = store.data_mut().ctx().table.get(&this)?.clone();
            let route = done.done(value);
            Ok((push(store.data_mut(), route)?,))
        },
    )
}

/// Lends the selection list styles in field order.
fn lend_select<T: CallbackView>(
    store: &mut T,
    styles: Vec<StyleEndpoint>,
) -> wasmtime::Result<SelectListTheme> {
    let table = &mut *store.ctx().table;
    let mut styles = styles.into_iter();
    let mut next = || -> wasmtime::Result<Resource<StyleEndpoint>> {
        let style = styles
            .next()
            .ok_or_else(|| wasmtime::format_err!("too few selection list styles"))?;
        Ok(table.push(style)?)
    };
    Ok(SelectListTheme {
        selected_prefix: next()?,
        selected_text: next()?,
        description: next()?,
        scroll_info: next()?,
        no_match: next()?,
    })
}

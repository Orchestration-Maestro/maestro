//! Canonical UI imports delegate through the existing typed local request pump.
use crate::{
    guest_calls::Callback,
    local_resources::SignalEndpoint,
    read_imports::{ReadEndpoint, add_routed_operand, operand},
    ui_reads::{Dialog, Endpoint, FactoryRequest, Request},
};
use wasmtime::{
    StoreContextMut,
    component::{Accessor, ComponentType, Lift, LinkerInstance, Resource},
};
use wasmtime_wasi::WasiView;

/// Installs every UI method declared by the canonical host interface.
pub(crate) fn add_to_linker<T: crate::callback_imports::CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    add_routed_operand(
        host,
        "[method]extension-ui-context.set-working-message",
        Request::WorkingMessage,
        operand::<T, Endpoint, Option<String>>,
    )?;
    add_routed_operand(
        host,
        "[method]extension-ui-context.set-working-visible",
        Request::WorkingVisible,
        operand::<T, Endpoint, bool>,
    )?;
    add_routed_operand(
        host,
        "[method]extension-ui-context.set-hidden-thinking-label",
        Request::HiddenThinkingLabel,
        operand::<T, Endpoint, Option<String>>,
    )?;
    add_routed_operand(
        host,
        "[method]extension-ui-context.set-title",
        Request::Title,
        operand::<T, Endpoint, String>,
    )?;
    host.func_wrap(
        "[method]extension-ui-context.set-working-indicator",
        |mut store: StoreContextMut<'_, T>,
         (resource, options): (Resource<Endpoint>, Option<Indicator>)| {
            let endpoint = store.data_mut().ctx().table.get(&resource)?.clone();
            let route =
                endpoint.route(|reply| Request::WorkingIndicator(options.map(Into::into), reply));
            Ok((crate::import_route::push(store.data_mut(), route)?,))
        },
    )?;
    crate::custom_imports::add_to_linker(host)?;
    editor(host)?;
    dialogs(host)?;
    widget_factory(host)?;
    notification(host)?;
    status(host)?;
    widget(host)
}

/// Keeps editor mutations on the same reentrant route as other native UI actions, and adds
/// terminal input listeners under the original extension.
fn editor<T: crate::callback_imports::CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    add_routed_operand(
        host,
        "[method]extension-ui-context.paste-to-editor",
        Request::PasteToEditor,
        operand::<T, Endpoint, String>,
    )?;
    add_routed_operand(
        host,
        "[method]extension-ui-context.set-editor-text",
        Request::SetEditorText,
        operand::<T, Endpoint, String>,
    )?;
    crate::read_imports::add_read(
        host,
        "[method]extension-ui-context.get-editor-text",
        Request::GetEditorText,
        crate::read_imports::resource::<T, Endpoint>,
    )?;
    host.func_wrap(
        "[method]extension-ui-context.on-terminal-input",
        |mut store: StoreContextMut<'_, T>,
         (resource, handler): (Resource<Endpoint>, Resource<Callback>)| {
            let callback = crate::callback_imports::retain(store.data_mut(), &handler)?;
            let api = store.data().api().clone();
            let endpoint = store.data_mut().ctx().table.get(&resource)?.clone();
            let result = endpoint.read(|reply| Request::OnTerminalInput(callback, api, reply));
            let result = match result {
                Ok(subscription) => Ok(store.data_mut().ctx().table.push(subscription)?),
                Err(error) => Err(error),
            };
            Ok((result,))
        },
    )
}

/// The original UI and dialog signal named by a dialog's borrowed resources.
fn dialog_owners<T: WasiView + 'static>(
    accessor: &Accessor<T>,
    resource: &Resource<Endpoint>,
    signal: Option<&Resource<SignalEndpoint>>,
) -> wasmtime::Result<(Endpoint, Option<SignalEndpoint>)> {
    accessor.with(|mut access| {
        let table = &access.get().ctx().table;
        let signal = signal
            .map(|signal| table.get(signal).cloned())
            .transpose()?;
        Ok((table.get(resource)?.clone(), signal))
    })
}

/// Forwards each dialog with its literal operands, signal and timeout to the original UI.
fn dialogs<T: WasiView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    host.func_wrap_concurrent(
        "[method]extension-ui-context.select",
        |accessor, (resource, title, options, signal, timeout): DialogOperands<Vec<String>>| {
            Box::pin(async move {
                let (ui, signal) = dialog_owners(accessor, &resource, signal.as_ref())?;
                let select = |reply| Dialog::Select(title, options, reply);
                Ok((ui.dialog(select, signal, timeout).await,))
            })
        },
    )?;
    host.func_wrap_concurrent(
        "[method]extension-ui-context.confirm",
        |accessor, (resource, title, message, signal, timeout): DialogOperands<String>| {
            Box::pin(async move {
                let (ui, signal) = dialog_owners(accessor, &resource, signal.as_ref())?;
                let confirm = |reply| Dialog::Confirm(title, message, reply);
                Ok((ui.dialog(confirm, signal, timeout).await,))
            })
        },
    )?;
    host.func_wrap_concurrent(
        "[method]extension-ui-context.input",
        |accessor,
         (resource, title, placeholder, signal, timeout): DialogOperands<Option<String>>| {
            Box::pin(async move {
                let (ui, signal) = dialog_owners(accessor, &resource, signal.as_ref())?;
                let input = |reply| Dialog::Input(title, placeholder, reply);
                Ok((ui.dialog(input, signal, timeout).await,))
            })
        },
    )?;
    host.func_wrap_concurrent(
        "[method]extension-ui-context.editor",
        |accessor, (resource, title, prefill): (Resource<Endpoint>, String, Option<String>)| {
            Box::pin(async move {
                let (ui, _) = dialog_owners(accessor, &resource, None)?;
                let editor = |reply| Dialog::Editor(title, prefill, reply);
                Ok((ui.dialog(editor, None, None).await,))
            })
        },
    )
}

/// A dialog's UI, title, own operand, borrowed signal and timeout.
type DialogOperands<O> = (
    Resource<Endpoint>,
    String,
    O,
    Option<Resource<SignalEndpoint>>,
    Option<f64>,
);

/// Shows the component an authored factory builds, which the native owner may build before
/// the import returns.
fn widget_factory<T: crate::callback_imports::CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    host.func_wrap(
        "[method]extension-ui-context.set-widget-factory",
        |mut store: StoreContextMut<'_, T>,
         (resource, key, factory, options): (
            Resource<Endpoint>,
            String,
            Resource<Callback>,
            Option<WidgetOptions>,
        )| {
            let factory = crate::callback_imports::retain(store.data_mut(), &factory)?;
            let api = store.data().api().clone();
            let endpoint = store.data_mut().ctx().table.get(&resource)?.clone();
            let options = options.map(Into::into);
            let route = endpoint.route(|reply| {
                Request::Factory(FactoryRequest::WidgetFactory(
                    key, factory, api, options, reply,
                ))
            });
            Ok((crate::import_route::push(store.data_mut(), route)?,))
        },
    )?;
    for (name, request) in [
        ("footer", FactoryRequest::Footer as OptionalFactory),
        ("header", FactoryRequest::Header),
        ("editor-component", FactoryRequest::EditorComponent),
    ] {
        optional_factory(
            host,
            &format!("[method]extension-ui-context.set-{name}"),
            request,
        )?;
    }
    host.func_wrap(
        "[method]extension-ui-context.add-autocomplete-provider",
        |mut store: StoreContextMut<'_, T>,
         (resource, factory): (Resource<Endpoint>, Resource<Callback>)| {
            let factory = crate::callback_imports::retain(store.data_mut(), &factory)?;
            let api = store.data().api().clone();
            let endpoint = store.data_mut().ctx().table.get(&resource)?.clone();
            let route = endpoint.route(|reply| {
                Request::Factory(FactoryRequest::AutocompleteProvider(factory, api, reply))
            });
            Ok((crate::import_route::push(store.data_mut(), route)?,))
        },
    )
}

/// The request of a setter that installs or removes an authored factory.
type OptionalFactory = fn(
    Option<crate::guest_calls::Handle>,
    crate::api_reads::Endpoint,
    std::sync::mpsc::Sender<Result<(), String>>,
) -> FactoryRequest;

/// Installs one setter of an optional authored factory, which the native owner may run before
/// the import returns.
fn optional_factory<T: crate::callback_imports::CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
    name: &str,
    request: OptionalFactory,
) -> wasmtime::Result<()> {
    host.func_wrap(
        name,
        move |mut store: StoreContextMut<'_, T>,
              (resource, factory): (Resource<Endpoint>, Option<Resource<Callback>>)| {
            let factory = crate::callback_imports::optional(store.data_mut(), factory.as_ref())?;
            let api = store.data().api().clone();
            let endpoint = store.data_mut().ctx().table.get(&resource)?.clone();
            let route = endpoint.route(|reply| Request::Factory(request(factory, api, reply)));
            Ok((crate::import_route::push(store.data_mut(), route)?,))
        },
    )
}

/// Forwards severity without replacing an omitted value with a default.
fn notification<T: WasiView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    host.func_wrap(
        "[method]extension-ui-context.notify",
        |mut store: StoreContextMut<'_, T>,
         (resource, message, kind): (Resource<Endpoint>, String, Option<Notification>)| {
            let endpoint = store.data_mut().ctx().table.get(&resource)?.clone();
            let route =
                endpoint.route(|reply| Request::Notify(message, kind.map(Into::into), reply));
            Ok((crate::import_route::push(store.data_mut(), route)?,))
        },
    )
}

/// Forwards exact status keys and independently absent text.
fn status<T: WasiView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    host.func_wrap(
        "[method]extension-ui-context.set-status",
        |mut store: StoreContextMut<'_, T>,
         (resource, key, text): (Resource<Endpoint>, String, Option<String>)| {
            let endpoint = store.data_mut().ctx().table.get(&resource)?.clone();
            let route = endpoint.route(|reply| Request::Status(key, text, reply));
            Ok((crate::import_route::push(store.data_mut(), route)?,))
        },
    )
}

/// Forwards fixed widget lines without interpreting them as a component factory.
fn widget<T: WasiView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    host.func_wrap(
        "[method]extension-ui-context.set-widget",
        |mut store: StoreContextMut<'_, T>,
         (resource, key, content, options): (
            Resource<Endpoint>,
            String,
            Option<Vec<String>>,
            Option<WidgetOptions>,
        )| {
            let endpoint = store.data_mut().ctx().table.get(&resource)?.clone();
            let route = endpoint
                .route(|reply| Request::Widget(key, content, options.map(Into::into), reply));
            Ok((crate::import_route::push(store.data_mut(), route)?,))
        },
    )
}

/// Canonical notification severity.
#[derive(ComponentType, Lift)]
#[component(enum)]
#[repr(u8)]
#[expect(
    dead_code,
    reason = "constructed by the Wasmtime Lift macro from wire values; host code only matches them"
)]
enum Notification {
    /// Informational message.
    #[component(name = "info")]
    Info,
    /// Warning message.
    #[component(name = "warning")]
    Warning,
    /// Error message.
    #[component(name = "error")]
    Error,
}
impl From<Notification> for maestro_extensions::types::NotificationType {
    fn from(value: Notification) -> Self {
        match value {
            Notification::Info => Self::Info,
            Notification::Warning => Self::Warning,
            Notification::Error => Self::Error,
        }
    }
}

/// Canonical indicator options keep empty frames distinct from omitted frames.
#[derive(ComponentType, Lift)]
#[component(record)]
struct Indicator {
    /// Supplied animation frames.
    frames: Option<Vec<String>>,
    /// Supplied frame interval, with no host-side default substitution.
    #[component(name = "interval-ms")]
    interval_ms: Option<f64>,
}
impl From<Indicator> for maestro_extensions::WorkingIndicatorOptions {
    fn from(value: Indicator) -> Self {
        Self {
            frames: value.frames,
            interval_ms: value.interval_ms,
        }
    }
}

/// Canonical fixed-widget placement.
#[derive(ComponentType, Lift)]
#[component(enum)]
#[repr(u8)]
#[expect(
    dead_code,
    reason = "constructed by the Wasmtime Lift macro from wire values; host code only matches them"
)]
enum Placement {
    /// Above the editor.
    #[component(name = "above-editor")]
    AboveEditor,
    /// Below the editor.
    #[component(name = "below-editor")]
    BelowEditor,
}
impl From<Placement> for maestro_extensions::WidgetPlacement {
    fn from(value: Placement) -> Self {
        match value {
            Placement::AboveEditor => Self::AboveEditor,
            Placement::BelowEditor => Self::BelowEditor,
        }
    }
}

/// Canonical optional widget settings.
#[derive(ComponentType, Lift)]
#[component(record)]
struct WidgetOptions {
    /// Missing placement leaves the original UI's policy unchanged.
    placement: Option<Placement>,
}
impl From<WidgetOptions> for maestro_extensions::ExtensionWidgetOptions {
    fn from(value: WidgetOptions) -> Self {
        Self {
            placement: value.placement.map(Into::into),
        }
    }
}

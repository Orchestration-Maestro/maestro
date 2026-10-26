//! Canonical UI imports delegate through the existing typed local request pump.
use crate::{
    read_imports::{ReadEndpoint, add_routed_operand, operand},
    ui_reads::{Endpoint, Request},
};
use wasmtime::{
    StoreContextMut,
    component::{ComponentType, Lift, LinkerInstance, Resource},
};
use wasmtime_wasi::WasiView;

/// Installs every UI method declared by the canonical host interface.
pub(crate) fn add_to_linker<T: WasiView + 'static>(
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
    editor(host)?;
    notification(host)?;
    status(host)?;
    widget(host)
}

/// Keeps editor mutations on the same reentrant route as other native UI actions.
fn editor<T: WasiView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
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

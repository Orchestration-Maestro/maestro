//! Canonical theme methods delegate through the original local request pump.
use crate::{
    read_imports::{ReadEndpoint, add_operand, add_read, operand, resource},
    theme_reads::{Endpoint, Request},
};
use std::sync::mpsc::Sender;
use wasmtime::{
    StoreContextMut,
    component::{ComponentType, LinkerInstance, Lower, Resource},
};
use wasmtime_wasi::WasiView;

/// Canonical color depth returned by the current native theme.
#[derive(Clone, Copy, ComponentType, Lower)]
#[component(enum)]
#[repr(u8)]
pub(crate) enum ColorMode {
    /// Direct RGB output.
    #[component(name = "truecolor")]
    Truecolor,
    /// Indexed terminal palette.
    #[component(name = "color256")]
    Color256,
}
impl From<maestro_extensions::ColorMode> for ColorMode {
    fn from(mode: maestro_extensions::ColorMode) -> Self {
        match mode {
            maestro_extensions::ColorMode::Truecolor => Self::Truecolor,
            maestro_extensions::ColorMode::Color256 => Self::Color256,
        }
    }
}

/// Installs standalone access to the widget host's live palette.
pub(crate) fn add_global<T: crate::api_imports::ApiView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    host.func_wrap(
        "global-theme",
        |mut store: StoreContextMut<'_, T>, (): ()| {
            let theme = store.data().api().0.request(|_, reply| {
                crate::local_resources::Message::Widgets(
                    crate::widget_reads::WidgetMessage::GlobalTheme(reply),
                )
            });
            let theme = match theme {
                Ok(theme) => Ok(store.data_mut().ctx().table.push(theme)?),
                Err(error) => Err(error),
            };
            Ok((theme,))
        },
    )
}

/// Installs every canonical method on the existing retained theme resource.
pub(crate) fn add_to_linker<T: WasiView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    for (method, request) in [
        (
            "name",
            Request::Name as fn(Sender<Result<Option<String>, String>>) -> Request,
        ),
        ("source-path", Request::SourcePath),
        ("source-info", Request::SourceInfo),
    ] {
        add_read(
            host,
            &format!("[method]theme.{method}"),
            request,
            resource::<T, Endpoint>,
        )?;
    }
    add_read(
        host,
        "[method]theme.get-color-mode",
        Request::ColorMode,
        resource::<T, Endpoint>,
    )?;
    add_operand(
        host,
        "[method]theme.set-source-info",
        Request::SetSourceInfo,
        operand::<T, Endpoint, Option<String>>,
    )?;
    for (method, request) in [
        (
            "bold",
            Request::Bold as fn(String, Sender<Result<String, String>>) -> Request,
        ),
        ("italic", Request::Italic),
        ("underline", Request::Underline),
        ("inverse", Request::Inverse),
        ("strikethrough", Request::Strikethrough),
        ("get-fg-ansi", Request::FgAnsi),
        ("get-bg-ansi", Request::BgAnsi),
        ("bash-mode-border", Request::BashModeBorder),
    ] {
        add_operand(
            host,
            &format!("[method]theme.{method}"),
            request,
            operand::<T, Endpoint, String>,
        )?;
    }
    colors(host)
}

/// Keeps foreground and background keys open, preserving exact strings and native errors.
fn colors<T: WasiView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    for (method, request) in [
        (
            "fg",
            Request::Fg as fn(String, String, Sender<Result<String, String>>) -> Request,
        ),
        ("bg", Request::Bg),
        ("thinking-border", Request::ThinkingBorder),
    ] {
        host.func_wrap(
            &format!("[method]theme.{method}"),
            move |mut store: StoreContextMut<'_, T>,
                  (resource, color, text): (Resource<Endpoint>, String, String)| {
                let endpoint = store.data_mut().ctx().table.get(&resource)?.clone();
                Ok((endpoint.read(|reply| request(color, text, reply)),))
            },
        )?;
    }
    Ok(())
}

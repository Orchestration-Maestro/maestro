//! Theme lookup, selection and tool expansion on the originally acquired UI.
use crate::{
    local_resources::{Registry, effects::ThemeEndpoint},
    read_imports::{ReadEndpoint, add_operand, add_read, add_routed_operand, operand, resource},
    ui_reads::{Endpoint, Request},
};
use maestro_extensions::{
    ExtensionUIContext, LiveTheme,
    types::{ThemeSelection, ThemeSetResult},
};
use std::sync::mpsc::Sender;
use wasmtime::{
    StoreContextMut,
    component::{ComponentType, LinkerInstance, Lower, Resource},
};
use wasmtime_wasi::WasiView;

/// Catchable native reply.
type Reply<T> = Sender<Result<T, String>>;

/// One theme or tool expansion request.
pub(crate) enum ThemeRequest {
    /// Retains the UI's live theme.
    Current(Reply<ThemeEndpoint>),
    /// Lists every available theme.
    All(Reply<Vec<Info>>),
    /// Loads a theme by name without selecting it.
    Load(String, Reply<Option<ThemeEndpoint>>),
    /// Selects a theme by name.
    SelectName(String, Reply<SetResult>),
    /// Selects a retained theme.
    Select(ThemeEndpoint, Reply<SetResult>),
    /// Reads the tool output expansion.
    Expanded(Reply<bool>),
    /// Changes the tool output expansion.
    Expand(bool, Reply<()>),
}

/// Canonical record of one available theme.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(crate) struct Info {
    /// Theme name.
    name: String,
    /// Source file, when the theme has one.
    path: Option<String>,
}

/// Canonical selection result.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(crate) struct SetResult {
    /// Whether the selection applied.
    success: bool,
    /// The host's message when it did not.
    error: Option<String>,
}
impl From<ThemeSetResult> for SetResult {
    fn from(result: ThemeSetResult) -> Self {
        Self {
            success: result.success,
            error: result.error,
        }
    }
}

impl ThemeRequest {
    /// Calls the original UI outside all capability-table borrows.
    pub(crate) fn answer(self, ui: &ExtensionUIContext, registry: &Registry) {
        let select = |selection| {
            ui.set_theme(selection)
                .map(Into::into)
                .map_err(|error| error.to_string())
        };
        match self {
            Self::Current(reply) => drop(reply.send(registry.theme(ui.theme()))),
            Self::All(reply) => {
                let themes = ui
                    .get_all_themes()
                    .map_err(|error| error.to_string())
                    .map(|themes| {
                        let info = themes.into_iter().map(|it| Info {
                            name: it.name,
                            path: it.path,
                        });
                        info.collect()
                    });
                drop(reply.send(themes));
            }
            Self::Load(name, reply) => {
                let loaded = ui
                    .get_theme(name)
                    .map_err(|error| error.to_string())
                    .and_then(|theme| {
                        let retain = |theme| registry.theme(LiveTheme::from_theme(theme));
                        theme.map(retain).transpose()
                    });
                drop(reply.send(loaded));
            }
            Self::SelectName(name, reply) => drop(reply.send(select(ThemeSelection::Name(name)))),
            Self::Select(theme, reply) => {
                let live = registry.live_theme(&theme);
                drop(reply.send(live.and_then(|live| select(ThemeSelection::Theme(live)))));
            }
            Self::Expanded(reply) => {
                drop(reply.send(ui.get_tools_expanded().map_err(|error| error.to_string())));
            }
            Self::Expand(expanded, reply) => {
                drop(
                    reply.send(
                        ui.set_tools_expanded(expanded)
                            .map_err(|error| error.to_string()),
                    ),
                );
            }
        }
    }

    /// Settles a released capability without leaving the Store caller waiting.
    pub(crate) fn reject(self, error: &str) {
        match self {
            Self::Current(reply) => refuse(&reply, error),
            Self::All(reply) => refuse(&reply, error),
            Self::Load(_, reply) => refuse(&reply, error),
            Self::SelectName(_, reply) | Self::Select(_, reply) => refuse(&reply, error),
            Self::Expanded(reply) => refuse(&reply, error),
            Self::Expand(_, reply) => refuse(&reply, error),
        }
    }
}

/// Settles `reply` with `error`.
fn refuse<T>(reply: &Reply<T>, error: &str) {
    let _ = reply.send(Err(error.to_owned()));
}

/// Installs theme lookup, selection and tool expansion on the canonical UI resource.
pub(crate) fn add_to_linker<T: WasiView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    host.func_wrap(
        "[method]extension-ui-context.theme",
        |mut store: StoreContextMut<'_, T>, (resource,): (Resource<Endpoint>,)| {
            let endpoint = store.data_mut().ctx().table.get(&resource)?.clone();
            let theme = endpoint.read(|reply| Request::Theme(ThemeRequest::Current(reply)));
            let pushed = match theme {
                Ok(theme) => Ok(store.data_mut().ctx().table.push(theme)?),
                Err(error) => Err(error),
            };
            Ok((pushed,))
        },
    )?;
    host.func_wrap(
        "[method]extension-ui-context.get-theme",
        |mut store: StoreContextMut<'_, T>, (resource, name): (Resource<Endpoint>, String)| {
            let endpoint = store.data_mut().ctx().table.get(&resource)?.clone();
            let theme = endpoint.read(|reply| Request::Theme(ThemeRequest::Load(name, reply)));
            let pushed = match theme {
                Ok(Some(theme)) => Ok(Some(store.data_mut().ctx().table.push(theme)?)),
                Ok(None) => Ok(None),
                Err(error) => Err(error),
            };
            Ok((pushed,))
        },
    )?;
    host.func_wrap(
        "[method]extension-ui-context.set-theme",
        |mut store: StoreContextMut<'_, T>,
         (resource, theme): (Resource<Endpoint>, Resource<ThemeEndpoint>)| {
            let table = store.data_mut().ctx().table;
            let (endpoint, theme) = (table.get(&resource)?.clone(), table.get(&theme)?.clone());
            let select = |reply| Request::Theme(ThemeRequest::Select(theme, reply));
            Ok((endpoint.read(select),))
        },
    )?;
    add_read(
        host,
        "[method]extension-ui-context.get-all-themes",
        |reply| Request::Theme(ThemeRequest::All(reply)),
        resource::<T, Endpoint>,
    )?;
    add_operand(
        host,
        "[method]extension-ui-context.set-theme-name",
        |name, reply| Request::Theme(ThemeRequest::SelectName(name, reply)),
        operand::<T, Endpoint, String>,
    )?;
    add_read(
        host,
        "[method]extension-ui-context.get-tools-expanded",
        |reply| Request::Theme(ThemeRequest::Expanded(reply)),
        resource::<T, Endpoint>,
    )?;
    add_routed_operand(
        host,
        "[method]extension-ui-context.set-tools-expanded",
        |expanded, reply| Request::Theme(ThemeRequest::Expand(expanded, reply)),
        operand::<T, Endpoint, bool>,
    )
}

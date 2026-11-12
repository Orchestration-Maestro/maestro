//! Canonical keybindings, hint and editor style imports delegate to the host's owners.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{
    bindings::maestro::extension::host::{
        HostKeybindingsManager, KeybindingConflict, KeybindingDefinition,
    },
    callback_imports::CallbackView,
    context_imports::add_resource,
    keys_reads::{Config, Hint, KeysBuild, KeysEndpoint, config, definitions},
    lifecycle::State,
    local_resources::Message,
    read_imports::{endpoint, forwards, release},
    widget_reads::{StyleEndpoint, WidgetMessage},
};
use wasmtime::{
    StoreContextMut,
    component::{ComponentType, LinkerInstance, Lower, Resource},
};

/// The lent selection list styles of an editor theme.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(crate) struct SelectListTheme {
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

/// The lent editor styles.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(crate) struct EditorTheme {
    /// The border.
    #[component(name = "border-color")]
    border_color: Resource<StyleEndpoint>,
    /// The completion list.
    #[component(name = "select-list")]
    select_list: SelectListTheme,
}

/// Keeps a new manager on the local owner and lends it.
fn keep<T: CallbackView>(
    store: &mut T,
    build: KeysBuild,
) -> wasmtime::Result<Result<Resource<KeysEndpoint>, String>> {
    let kept = store
        .api()
        .0
        .request(|_, reply| Message::Widgets(WidgetMessage::NewKeys(build, reply)));
    Ok(match kept {
        Ok(keys) => Ok(store.ctx().table.push(keys)?),
        Err(error) => Err(error),
    })
}

/// Answers a hint on the local owner.
fn hint<T: CallbackView>(store: &T, hint: Hint) -> Result<String, String> {
    store
        .api()
        .0
        .request(|_, reply| Message::Widgets(WidgetMessage::Hint(hint, reply)))
}

/// Installs the manager constructors, the hints and the editor styles.
pub(crate) fn add_to_linker<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    constructors(host)?;
    hints(host)
}

/// Installs the manager constructors.
fn constructors<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    host.func_wrap(
        "new-keybindings-manager",
        |mut store: StoreContextMut<'_, T>, (user, path): (Config, Option<String>)| {
            Ok((keep(
                store.data_mut(),
                KeysBuild::Application(config(user), path),
            )?,))
        },
    )?;
    host.func_wrap(
        "create-keybindings-manager",
        |mut store: StoreContextMut<'_, T>, (dir,): (Option<String>,)| {
            Ok((keep(store.data_mut(), KeysBuild::Created(dir))?,))
        },
    )?;
    host.func_wrap(
        "new-keybindings-with-definitions",
        |mut store: StoreContextMut<'_, T>,
         (defined, user): (Vec<(String, KeybindingDefinition)>, Config)| {
            let build = KeysBuild::Definitions(definitions(defined), config(user));
            Ok((keep(store.data_mut(), build)?,))
        },
    )?;
    host.func_wrap(
        "get-keybindings",
        |mut store: StoreContextMut<'_, T>, (): ()| {
            Ok((keep(store.data_mut(), KeysBuild::Global)?,))
        },
    )
}

/// Declares the methods that run one endpoint operation.
macro_rules! reads {
    ($($method:ident($($operand:ident: $kind:ty),*) -> $result:ty;)+) => {
        $(fn $method(
            &mut self,
            this: Resource<KeysEndpoint>,
            $($operand: $kind),*
        ) -> wasmtime::Result<Result<$result, String>> {
            Ok(endpoint(self, &this)?.$method($($operand),*))
        })+
    };
}

impl HostKeybindingsManager for State {
    reads! {
        matches(data: String, keybinding: String) -> bool;
        get_keys(keybinding: String) -> Vec<String>;
        get_definition(keybinding: String) -> Option<KeybindingDefinition>;
        get_conflicts() -> Vec<KeybindingConflict>;
        get_user_bindings() -> Config;
        get_resolved_bindings() -> Config;
        reload() -> ();
    }

    fn set_user_bindings(
        &mut self,
        this: Resource<KeysEndpoint>,
        user_bindings: Config,
    ) -> wasmtime::Result<Result<(), String>> {
        Ok(endpoint(self, &this)?.set_user_bindings(config(user_bindings)))
    }

    fn get_effective_config(
        &mut self,
        this: Resource<KeysEndpoint>,
    ) -> wasmtime::Result<Result<Config, String>> {
        Ok(endpoint(self, &this)?.get_resolved_bindings())
    }

    fn drop(&mut self, this: Resource<KeysEndpoint>) -> wasmtime::Result<()> {
        release(self, this)
    }
}

/// Forwards every keybindings manager method to its trait implementation.
pub(crate) fn add_manager(host: &mut LinkerInstance<'_, State>) -> wasmtime::Result<()> {
    add_resource::<State, KeysEndpoint>(host, "keybindings-manager")?;
    forwards! { host, "keybindings-manager", HostKeybindingsManager, KeysEndpoint,
        "matches" => matches(data: String, keybinding: String);
        "get-keys" => get_keys(keybinding: String);
        "get-definition" => get_definition(keybinding: String);
        "get-conflicts" => get_conflicts();
        "set-user-bindings" => set_user_bindings(user_bindings: Config);
        "get-user-bindings" => get_user_bindings();
        "get-resolved-bindings" => get_resolved_bindings();
        "reload" => reload();
        "get-effective-config" => get_effective_config();
    }
    Ok(())
}

/// Installs the hints and the lent editor styles.
fn hints<T: CallbackView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    host.func_wrap(
        "key-text",
        |store: StoreContextMut<'_, T>, (id,): (String,)| Ok((hint(store.data(), Hint::Text(id)),)),
    )?;
    host.func_wrap(
        "key-hint",
        |store: StoreContextMut<'_, T>, (id, text): (String, String)| {
            Ok((hint(store.data(), Hint::Keybinding(id, text)),))
        },
    )?;
    host.func_wrap(
        "raw-key-hint",
        |store: StoreContextMut<'_, T>, (key, text): (String, String)| {
            Ok((hint(store.data(), Hint::Raw(key, text)),))
        },
    )?;
    host.func_wrap(
        "get-editor-theme",
        |mut store: StoreContextMut<'_, T>, (): ()| {
            let lent = store
                .data()
                .api()
                .0
                .request(|_, reply| Message::Widgets(WidgetMessage::LendEditor(reply)));
            let lent = match lent {
                Ok(lent) => Ok(lend(store.data_mut().ctx().table, lent)?),
                Err(error) => Err(error),
            };
            Ok((lent,))
        },
    )
}

/// Lends the editor border and selection list styles in field order.
pub(crate) fn lend(
    table: &mut wasmtime::component::ResourceTable,
    (border, styles): (StyleEndpoint, Vec<StyleEndpoint>),
) -> wasmtime::Result<EditorTheme> {
    let border_color = table.push(border)?;
    let mut styles = styles.into_iter();
    let mut next = || -> wasmtime::Result<Resource<StyleEndpoint>> {
        let style = styles
            .next()
            .ok_or_else(|| wasmtime::format_err!("too few editor styles"))?;
        Ok(table.push(style)?)
    };
    Ok(EditorTheme {
        border_color,
        select_list: SelectListTheme {
            selected_prefix: next()?,
            selected_text: next()?,
            description: next()?,
            scroll_info: next()?,
            no_match: next()?,
        },
    })
}

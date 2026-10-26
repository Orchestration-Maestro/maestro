//! Canonical keybindings, hint and editor style imports delegate to the host's owners.
use crate::{
    callback_imports::CallbackView,
    keys_reads::{
        Config, Definition, Hint, KeysBuild, KeysEndpoint, KeysRequest, config, definitions,
    },
    local_resources::Message,
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

/// The manager behind a borrowed resource.
fn manager<T: CallbackView>(
    store: &mut T,
    this: &Resource<KeysEndpoint>,
) -> wasmtime::Result<KeysEndpoint> {
    Ok(store.ctx().table.get(this)?.clone())
}

/// Installs the manager constructors, its methods, the hints and the editor styles.
pub(crate) fn add_to_linker<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    crate::context_imports::add_resource::<T, KeysEndpoint>(host, "keybindings-manager")?;
    constructors(host)?;
    reads(host)?;
    writes(host)?;
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
         (defined, user): (Vec<(String, Definition)>, Config)| {
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

/// Installs the reads of a manager.
fn reads<T: CallbackView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    host.func_wrap(
        "[method]keybindings-manager.matches",
        |mut store: StoreContextMut<'_, T>,
         (this, data, id): (Resource<KeysEndpoint>, String, String)| {
            let manager = manager(store.data_mut(), &this)?;
            Ok((manager.read(|reply| KeysRequest::Matches(data, id, reply)),))
        },
    )?;
    host.func_wrap(
        "[method]keybindings-manager.get-keys",
        |mut store: StoreContextMut<'_, T>, (this, id): (Resource<KeysEndpoint>, String)| {
            let manager = manager(store.data_mut(), &this)?;
            Ok((manager.read(|reply| KeysRequest::Keys(id, reply)),))
        },
    )?;
    host.func_wrap(
        "[method]keybindings-manager.get-definition",
        |mut store: StoreContextMut<'_, T>, (this, id): (Resource<KeysEndpoint>, String)| {
            let manager = manager(store.data_mut(), &this)?;
            Ok((manager.read(|reply| KeysRequest::Definition(id, reply)),))
        },
    )?;
    host.func_wrap(
        "[method]keybindings-manager.get-conflicts",
        |mut store: StoreContextMut<'_, T>, (this,): (Resource<KeysEndpoint>,)| {
            Ok((manager(store.data_mut(), &this)?.read(KeysRequest::Conflicts),))
        },
    )?;
    for method in [
        "get-user-bindings",
        "get-resolved-bindings",
        "get-effective-config",
    ] {
        let user = method == "get-user-bindings";
        host.func_wrap(
            &format!("[method]keybindings-manager.{method}"),
            move |mut store: StoreContextMut<'_, T>, (this,): (Resource<KeysEndpoint>,)| {
                let manager = manager(store.data_mut(), &this)?;
                let request = if user {
                    KeysRequest::UserBindings
                } else {
                    KeysRequest::ResolvedBindings
                };
                Ok((manager.read(request),))
            },
        )?;
    }
    Ok(())
}

/// Installs the changes of a manager.
fn writes<T: CallbackView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    host.func_wrap(
        "[method]keybindings-manager.set-user-bindings",
        |mut store: StoreContextMut<'_, T>, (this, user): (Resource<KeysEndpoint>, Config)| {
            let manager = manager(store.data_mut(), &this)?;
            Ok((manager.read(|reply| KeysRequest::SetUserBindings(config(user), reply)),))
        },
    )?;
    host.func_wrap(
        "[method]keybindings-manager.reload",
        |mut store: StoreContextMut<'_, T>, (this,): (Resource<KeysEndpoint>,)| {
            Ok((manager(store.data_mut(), &this)?.read(KeysRequest::Reload),))
        },
    )
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

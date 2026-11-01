//! Canonical named selection borrows the existing callback resources.
use crate::{callback_imports::CallbackView, guest_calls::Callback};
use wasmtime::{
    StoreContextMut,
    component::{ComponentType, Lift, LinkerInstance, Resource},
};

/// Typed wire controls keep callback capabilities outside JSON.
#[derive(ComponentType, Lift)]
#[component(record)]
struct Options {
    /// Optional native read image policy.
    #[component(name = "read-auto-resize-images")]
    read_auto_resize_images: Option<bool>,
    /// Independent native shell controls.
    #[component(name = "bash-controls")]
    bash_controls: crate::local_resources::builtin_bash::Controls,
    /// Original read effects.
    #[component(name = "read-operations")]
    read: Option<Resource<Callback>>,
    /// Original shell effects.
    #[component(name = "bash-operations")]
    bash: Option<Resource<Callback>>,
    /// Original edit effects.
    #[component(name = "edit-operations")]
    edit: Option<Resource<Callback>>,
    /// Original write effects.
    #[component(name = "write-operations")]
    write: Option<Resource<Callback>>,
    /// Original content-search effects.
    #[component(name = "grep-operations")]
    grep: Option<Resource<Callback>>,
    /// Original file-search effects.
    #[component(name = "find-operations")]
    find: Option<Resource<Callback>>,
    /// Original directory effects.
    #[component(name = "ls-operations")]
    ls: Option<Resource<Callback>>,
    /// Original synchronous shell interception.
    #[component(name = "spawn-hook")]
    hook: Option<Resource<Callback>>,
}

/// Installs the native selector without a second factory registry.
pub(crate) fn add_to_linker<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    add_preset(host)?;
    host.func_wrap(
        "create-tool",
        |mut store: StoreContextMut<'_, T>, (name, cwd, options): (String, String, Options)| {
            let options = retain_options(store.data_mut(), options)?;
            let result = store.data().api().create_selected(name, cwd, options);
            let result = match result {
                Ok(tool) => Ok(store.data_mut().ctx().table.push(tool)?),
                Err(error) => Err(error),
            };
            Ok((result,))
        },
    )
}

/// Retains borrowed observations identically for named and preset admissions.
fn retain_options<T: CallbackView>(
    store: &mut T,
    options: Options,
) -> wasmtime::Result<crate::local_resources::builtin_selection::Options> {
    let mut retain = |callback: Option<Resource<Callback>>| {
        callback
            .as_ref()
            .map(|callback| crate::callback_imports::retain_object(store, callback))
            .transpose()
    };
    Ok(crate::local_resources::builtin_selection::Options {
        read_resize: options.read_auto_resize_images,
        read: retain(options.read)?,
        bash: crate::local_resources::builtin_bash::Options {
            controls: options.bash_controls,
            operations: retain(options.bash)?,
            hook: retain(options.hook)?,
        },
        edit: retain(options.edit)?,
        write: retain(options.write)?,
        grep: retain(options.grep)?,
        find: retain(options.find)?,
        ls: retain(options.ls)?,
    })
}

/// Installs the native ordered preset return without projecting keys through JSON.
fn add_preset<T: CallbackView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    host.func_wrap(
        "create-tool-preset",
        |mut store: StoreContextMut<'_, T>,
         (preset, cwd, options): (
            crate::local_resources::builtin_selection::Preset,
            String,
            Options,
        )| {
            let options = retain_options(store.data_mut(), options)?;
            let result = store.data().api().create_preset(preset, cwd, options);
            let result = match result {
                Ok(tools) => Ok(tools
                    .into_iter()
                    .map(|(name, tool)| Ok((name, store.data_mut().ctx().table.push(tool)?)))
                    .collect::<wasmtime::Result<Vec<_>>>()?),
                Err(error) => Err(error),
            };
            Ok((result,))
        },
    )
}

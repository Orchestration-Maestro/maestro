//! Canonical API imports use the original facade through the single local pump.
use crate::api_reads::{Endpoint, Request};
use crate::read_imports::ReadEndpoint;
use crate::read_imports::{add_read, add_routed_operand};
use maestro_extensions::types;
use wasmtime::{
    StoreContextMut,
    component::{ComponentType, Lift, LinkerInstance, Lower},
};
use wasmtime_wasi::WasiView;

/// Installs the one synchronous load-time input route on the original local API.
fn add_load_time<T: crate::callback_imports::CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    host.func_wrap(
        "contribute-load-time",
        |store: StoreContextMut<'_, T>, (payload,): (String,)| {
            Ok((store
                .data()
                .api()
                .read(|reply| Request::ContributeLoadTime(payload, reply)),))
        },
    )
}

/// Store access to its original API endpoint, never to a local registration facade.
pub(crate) trait ApiView: WasiView {
    /// Borrows the Store-safe endpoint retained during initialization.
    fn api(&self) -> &Endpoint;
}

/// Installs provider registration and runtime action imports.
pub(crate) fn add_to_linker<T: crate::callback_imports::CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    crate::local_resources::host_operations::add_to_linker(host)?;
    add_values(host)?;
    add_load_time(host)?;
    add_providers(host)?;
    for (name, request) in [
        ("append-entry", Request::Append as fn(_, _, _) -> _),
        ("send-message", Request::SendMessage),
        ("send-user-message", Request::SendUserMessage),
        ("set-label", Request::Label),
    ] {
        host.func_wrap(
            name,
            move |mut store: StoreContextMut<'_, T>, (value, options): (String, Option<String>)| {
                let route = store
                    .data()
                    .api()
                    .route(|reply| request(value, options, reply));
                Ok((crate::import_route::push(store.data_mut(), route)?,))
            },
        )?;
    }
    host.func_wrap(
        "register-flag",
        |store: StoreContextMut<'_, T>, (name, options): (String, FlagOptions)| {
            Ok((store
                .data()
                .api()
                .read(|reply| Request::RegisterFlag(name, options.into(), reply)),))
        },
    )?;
    host.func_wrap(
        "get-flag",
        |store: StoreContextMut<'_, T>, (name,): (String,)| {
            Ok((store
                .data()
                .api()
                .read(|reply| Request::Flag(name, reply))
                .map(|value| value.map(FlagValue::from)),))
        },
    )?;
    host.func_wrap_concurrent("set-model", |accessor, (model,): (String,)| {
        Box::pin(async move {
            let endpoint = accessor.with(|mut access| access.get().api().clone());
            let result = match crate::writer_reads::decode(&model) {
                Ok(model) => endpoint.set_model(model).await,
                Err(error) => Err(error),
            };
            Ok((result,))
        })
    })?;
    Ok(())
}

/// Registers the facade's primitive getters and setters.
fn add_values<T: ApiView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    add_read(
        host,
        "get-session-name",
        Request::SessionName,
        |store, ()| Ok(store.api().clone()),
    )?;
    add_read(
        host,
        "get-active-tools",
        Request::ActiveTools,
        |store, ()| Ok(store.api().clone()),
    )?;
    add_read(host, "get-commands", Request::Commands, |store, ()| {
        Ok(store.api().clone())
    })?;
    add_read(
        host,
        "get-thinking-level",
        Request::Thinking,
        |store, ()| Ok(store.api().clone()),
    )?;
    add_routed_operand(
        host,
        "set-session-name",
        Request::SetSessionName,
        |store, (value,)| Ok((store.api().clone(), value)),
    )?;
    add_routed_operand(
        host,
        "set-active-tools",
        Request::SetActiveTools,
        |store, (value,)| Ok((store.api().clone(), value)),
    )?;
    add_routed_operand(
        host,
        "set-thinking-level",
        Request::SetThinking,
        |store, (value,)| Ok((store.api().clone(), value)),
    )?;
    Ok(())
}

/// Declared flag kind stays independent of the default's variant.
#[derive(ComponentType, Lift)]
#[component(enum)]
#[repr(u8)]
#[expect(
    dead_code,
    reason = "constructed by the Wasmtime Lift macro from wire values; host code only matches them"
)]
enum FlagType {
    /// Boolean parser declaration.
    #[component(name = "boolean")]
    Boolean,
    /// String parser declaration.
    #[component(name = "string")]
    String,
}

/// Canonical flag values retain false and empty strings.
#[derive(ComponentType, Lift, Lower)]
#[component(variant)]
enum FlagValue {
    /// Supplied boolean.
    #[component(name = "boolean")]
    Boolean(bool),
    /// Supplied string.
    #[component(name = "string")]
    String(String),
}

impl From<FlagValue> for types::FlagValue {
    fn from(value: FlagValue) -> Self {
        match value {
            FlagValue::Boolean(value) => Self::Boolean(value),
            FlagValue::String(value) => Self::String(value),
        }
    }
}

impl From<types::FlagValue> for FlagValue {
    fn from(value: types::FlagValue) -> Self {
        match value {
            types::FlagValue::Boolean(value) => Self::Boolean(value),
            types::FlagValue::String(value) => Self::String(value),
        }
    }
}

/// Registration fields preserve independent optional description and default values.
#[derive(ComponentType, Lift)]
#[component(record)]
struct FlagOptions {
    /// Authored help text, including an explicitly empty value.
    description: Option<String>,
    /// Declared parser kind.
    #[component(name = "type")]
    kind: FlagType,
    /// Supplied default, irrespective of the parser kind.
    default: Option<FlagValue>,
}

impl From<FlagOptions> for types::ExtensionFlagOptions {
    fn from(options: FlagOptions) -> Self {
        Self {
            description: options.description,
            r#type: match options.kind {
                FlagType::Boolean => types::FlagType::Boolean,
                FlagType::String => types::FlagType::String,
            },
            default: options.default.map(types::FlagValue::from),
        }
    }
}

/// Links individual provider mutations to their original registration facade.
fn add_providers<T: crate::callback_imports::CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    host.func_wrap(
        "register-provider",
        |mut store: StoreContextMut<'_, T>,
         (name, config, callbacks): (String, String, crate::provider_registration::Wire)| {
            let callbacks =
                crate::provider_registration::retain(store.data_mut(), &name, callbacks)?;
            let route = store
                .data()
                .api()
                .route(|reply| Request::RegisterProvider(name, config, callbacks, reply));
            Ok((crate::import_route::push(store.data_mut(), route)?,))
        },
    )?;
    crate::read_imports::add_routed_operand(
        host,
        "unregister-provider",
        Request::UnregisterProvider,
        |store, (name,)| Ok((store.api().clone(), name)),
    )
}

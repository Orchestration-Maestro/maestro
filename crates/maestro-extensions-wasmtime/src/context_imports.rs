//! Canonical primitive imports sharing the original local context endpoint.
use crate::read_imports::ReadEndpoint;
use crate::read_imports::{add_read, add_routed};
use crate::{
    context_reads::{Endpoint, Request},
    local_resources::{SignalEndpoint, UiEndpoint},
};
use wasmtime::StoreContextMut;
use wasmtime::component::{ComponentType, Lift, LinkerInstance, Lower, Resource, ResourceType};
use wasmtime_wasi::WasiView;

/// The independently retained ordinary callback context.
pub(crate) struct Context(pub(crate) Endpoint);

/// The independently retained command context, with a distinct canonical identity.
pub(crate) struct CommandContext(pub(crate) Endpoint);

/// Common primitive operations without merging the two resource identities.
trait ContextResource: Send + 'static {
    /// Returns the endpoint of the original local context.
    fn endpoint(&self) -> &Endpoint;
}

impl ContextResource for Context {
    fn endpoint(&self) -> &Endpoint {
        &self.0
    }
}

impl ContextResource for CommandContext {
    fn endpoint(&self) -> &Endpoint {
        &self.0
    }
}

/// Owned canonical projection of the domain's optional usage estimate.
#[derive(ComponentType, Lift, Lower)]
#[component(record)]
struct ContextUsage {
    /// Unknown token estimates stay absent.
    tokens: Option<f64>,
    /// Capacity supplied by the domain.
    #[component(name = "context-window")]
    context_window: f64,
    /// Unknown percentages stay absent.
    percent: Option<f64>,
}

impl From<maestro_extensions::ContextUsage> for ContextUsage {
    fn from(usage: maestro_extensions::ContextUsage) -> Self {
        Self {
            tokens: usage.tokens,
            context_window: usage.context_window,
            percent: usage.percent,
        }
    }
}

/// Installs guarded context primitives and their independent UI and signal resources.
pub(crate) fn add_to_linker<T: crate::callback_imports::CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    add_resource::<T, UiEndpoint>(host, "extension-ui-context")?;
    add_resource::<T, SignalEndpoint>(host, "abort-signal")?;
    add_resource::<T, crate::local_resources::effects::TuiEndpoint>(host, "tui")?;
    add_resource::<T, crate::local_resources::effects::InvalidatorEndpoint>(
        host,
        "tool-invalidator",
    )?;
    host.func_wrap(
        "[method]tool-invalidator.invalidate",
        |mut store: wasmtime::StoreContextMut<'_, T>,
         (resource,): (Resource<crate::local_resources::effects::InvalidatorEndpoint>,)| {
            let invalidator = store.data_mut().ctx().table.get(&resource)?.clone();
            Ok((invalidator.invalidate(),))
        },
    )?;
    host.func_wrap(
        "[method]abort-signal.aborted",
        |mut store: StoreContextMut<'_, T>, (resource,): (Resource<SignalEndpoint>,)| {
            let signal = store.data_mut().ctx().table.get(&resource)?.clone();
            Ok((signal.aborted().map_err(wasmtime::Error::msg)?,))
        },
    )?;
    host.func_wrap(
        "[method]abort-signal.retain",
        |mut store: StoreContextMut<'_, T>, (signal,): (Resource<SignalEndpoint>,)| {
            let table = store.data_mut().ctx().table;
            let signal = table.get(&signal)?.clone();
            Ok((table.push(signal)?,))
        },
    )?;
    add_context::<T, Context>(host, "context")?;
    add_context::<T, CommandContext>(host, "command-context")?;
    Ok(())
}

/// Registers actual resource destruction without a placeholder drop handler.
pub(crate) fn add_resource<T: WasiView + 'static, R: Send + 'static>(
    host: &mut LinkerInstance<'_, T>,
    name: &str,
) -> wasmtime::Result<()> {
    host.resource(name, ResourceType::host::<R>(), |mut store, key| {
        store
            .data_mut()
            .ctx()
            .table
            .delete(Resource::<R>::new_own(key))?;
        Ok(())
    })?;
    Ok(())
}

/// Registers each common operation once for both distinct context identities.
fn add_context<T: crate::callback_imports::CallbackView + 'static, R: ContextResource>(
    host: &mut LinkerInstance<'_, T>,
    name: &str,
) -> wasmtime::Result<()> {
    add_resource::<T, R>(host, name)?;
    add_retained::<T, R, UiEndpoint>(host, name, "ui", Request::Ui)?;
    add_retained::<T, R, crate::history_reads::Endpoint>(
        host,
        name,
        "session-manager",
        Request::SessionManager,
    )?;
    add_retained::<T, R, crate::catalog_reads::Endpoint>(
        host,
        name,
        "model-registry",
        Request::ModelRegistry,
    )?;
    add_signal::<T, R>(host, name)?;
    add_compact::<T, R>(host, name)?;
    add_primitives::<T, R>(host, name)?;
    host.func_wrap(
        &format!("[method]{name}.get-context-usage"),
        |mut store: StoreContextMut<'_, T>, (resource,): (Resource<R>,)| {
            let endpoint = store
                .data_mut()
                .ctx()
                .table
                .get(&resource)?
                .endpoint()
                .clone();
            Ok((endpoint
                .read(Request::Usage)
                .map(|usage| usage.map(ContextUsage::from)),))
        },
    )?;
    Ok(())
}

/// Registers no-operand primitives on both guarded context identities.
fn add_primitives<T: WasiView + 'static, R: ContextResource>(
    host: &mut LinkerInstance<'_, T>,
    name: &str,
) -> wasmtime::Result<()> {
    add_read(
        host,
        &format!("[method]{name}.cwd"),
        Request::Cwd,
        endpoint::<T, R>,
    )?;
    add_read(
        host,
        &format!("[method]{name}.model"),
        Request::Model,
        endpoint::<T, R>,
    )?;
    add_read(
        host,
        &format!("[method]{name}.get-system-prompt"),
        Request::SystemPrompt,
        endpoint::<T, R>,
    )?;
    add_read(
        host,
        &format!("[method]{name}.has-ui"),
        Request::HasUi,
        endpoint::<T, R>,
    )?;
    add_read(
        host,
        &format!("[method]{name}.is-idle"),
        Request::IsIdle,
        endpoint::<T, R>,
    )?;
    add_read(
        host,
        &format!("[method]{name}.has-pending-messages"),
        Request::HasPendingMessages,
        endpoint::<T, R>,
    )?;
    add_routed(
        host,
        &format!("[method]{name}.abort"),
        Request::Abort,
        endpoint::<T, R>,
    )?;
    add_routed(
        host,
        &format!("[method]{name}.shutdown"),
        Request::Shutdown,
        endpoint::<T, R>,
    )?;
    Ok(())
}

/// Preserves absence while retaining a selected signal independently of its context.
fn add_signal<T: WasiView + 'static, R: ContextResource>(
    host: &mut LinkerInstance<'_, T>,
    name: &str,
) -> wasmtime::Result<()> {
    host.func_wrap(
        &format!("[method]{name}.signal"),
        |mut store: StoreContextMut<'_, T>, (resource,): (Resource<R>,)| {
            let endpoint = store
                .data_mut()
                .ctx()
                .table
                .get(&resource)?
                .endpoint()
                .clone();
            let result = match endpoint.read(Request::Signal) {
                Ok(signal) => Ok(signal
                    .map(|signal| store.data_mut().ctx().table.push(signal))
                    .transpose()?),
                Err(error) => Err(error),
            };
            Ok((result,))
        },
    )?;
    Ok(())
}

/// Inserts the actual independently retained capability into the Store resource table.
fn add_retained<T, R, V>(
    host: &mut LinkerInstance<'_, T>,
    name: &str,
    method: &str,
    request: fn(std::sync::mpsc::Sender<Result<V, String>>) -> Request,
) -> wasmtime::Result<()>
where
    T: WasiView + 'static,
    R: ContextResource,
    V: Send + 'static,
{
    host.func_wrap(
        &format!("[method]{name}.{method}"),
        move |mut store: StoreContextMut<'_, T>, (resource,): (Resource<R>,)| {
            let endpoint = store
                .data_mut()
                .ctx()
                .table
                .get(&resource)?
                .endpoint()
                .clone();
            let result = match endpoint.read(request) {
                Ok(capability) => Ok(store.data_mut().ctx().table.push(capability)?),
                Err(error) => Err(error),
            };
            Ok((result,))
        },
    )?;
    Ok(())
}

/// Complete canonical compaction operands, including independently optional callbacks.
type CompactArgs<R> = (
    Resource<R>,
    Option<crate::compact_calls::Data>,
    Option<Resource<crate::guest_calls::Callback>>,
    Option<Resource<crate::guest_calls::Callback>>,
);

/// Preserves optional records while retaining the actual authored notification callbacks.
fn add_compact<T: crate::callback_imports::CallbackView + 'static, R: ContextResource>(
    host: &mut LinkerInstance<'_, T>,
    name: &str,
) -> wasmtime::Result<()> {
    host.func_wrap(
        &format!("[method]{name}.compact"),
        |mut store: wasmtime::StoreContextMut<'_, T>,
         (resource, data, complete, error): CompactArgs<R>| {
            let endpoint = store
                .data_mut()
                .ctx()
                .table
                .get(&resource)?
                .endpoint()
                .clone();
            let options = crate::compact_calls::Compact {
                data,
                on_complete: crate::callback_imports::optional(
                    store.data_mut(),
                    complete.as_ref(),
                )?,
                on_error: crate::callback_imports::optional(store.data_mut(), error.as_ref())?,
                api: store.data().api().clone(),
            };
            let route = endpoint.route(|reply| Request::Compact(options, reply));
            Ok((crate::import_route::push(store.data_mut(), route)?,))
        },
    )?;
    Ok(())
}

/// Selects the original endpoint without merging ordinary and command resource identities.
fn endpoint<T: WasiView, R: ContextResource>(
    store: &mut T,
    (resource,): (Resource<R>,),
) -> wasmtime::Result<Endpoint> {
    Ok(store.ctx().table.get(&resource)?.endpoint().clone())
}

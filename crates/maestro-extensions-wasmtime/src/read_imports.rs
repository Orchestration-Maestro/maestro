//! Canonical read shims share one typed request path to each original native capability.
use crate::local_resources::Message;
use std::sync::mpsc::Sender;
use wasmtime::{
    StoreContextMut,
    component::{ComponentNamedList, ComponentType, Lift, LinkerInstance, Lower, Resource},
};
use wasmtime_wasi::WasiView;

/// Typed synchronous request service supplied by an original local capability.
pub(crate) trait ReadEndpoint: Clone + Send + 'static {
    /// Requests accepted by this capability's original owner.
    type Request;
    /// Sends a typed request through the extension's sole local receiver.
    fn read<T>(
        &self,
        request: impl FnOnce(Sender<Result<T, String>>) -> Self::Request,
    ) -> Result<T, String>;
    /// Sends a request whose native code may call back into this instance.
    fn route<T: Into<crate::import_route::Value> + Send + 'static>(
        &self,
        request: impl FnOnce(Sender<Result<T, String>>) -> Self::Request,
    ) -> crate::import_route::Route;
}
/// Declares the request envelope without copying the endpoint's transport implementation.
macro_rules! endpoints {
    ($($module:ident => $variant:ident),+ $(,)?) => {$(
        impl ReadEndpoint for crate::$module::Endpoint {
            type Request = crate::$module::Request;
            fn read<T>(&self, request: impl FnOnce(Sender<Result<T, String>>) -> Self::Request) -> Result<T, String> {
                self.0.request(|id, reply| Message::$variant(id, request(reply)))
            }
            fn route<T: Into<crate::import_route::Value> + Send + 'static>(&self, request: impl FnOnce(Sender<Result<T, String>>) -> Self::Request) -> crate::import_route::Route {
                self.0.route(|id, reply| Message::$variant(id, request(reply)))
            }
        }
    )+};
}
endpoints! { oauth_reads => OAuthProvider, builtin_reads => Builtin, api_reads => Api, context_reads => Read, history_reads => History,
writer_reads => Writer, credential_reads => Credentials, tree_reads => Node, catalog_reads => Catalog, theme_reads => Theme }

/// Clones an original resource endpoint before invoking its native owner.
pub(crate) fn resource<T: WasiView, E: ReadEndpoint>(
    store: &mut T,
    (resource,): (Resource<E>,),
) -> wasmtime::Result<E> {
    Ok(store.ctx().table.get(&resource)?.clone())
}
/// Separates an unchanged canonical operand from its original resource endpoint.
pub(crate) fn operand<T: WasiView, E: ReadEndpoint, A>(
    store: &mut T,
    (resource, value): (Resource<E>, A),
) -> wasmtime::Result<(E, A)> {
    Ok((store.ctx().table.get(&resource)?.clone(), value))
}
/// Installs one no-operand read for a facade or a retained capability.
pub(crate) fn add_read<T, E, P, V>(
    host: &mut LinkerInstance<'_, T>,
    name: &str,
    request: fn(Sender<Result<V, String>>) -> E::Request,
    endpoint: fn(&mut T, P) -> wasmtime::Result<E>,
) -> wasmtime::Result<()>
where
    T: WasiView + 'static,
    E: ReadEndpoint,
    P: ComponentNamedList + Lift + 'static,
    V: ComponentType + Lower + Send + 'static,
{
    host.func_wrap(name, move |mut store: StoreContextMut<'_, T>, params: P| {
        Ok((endpoint(store.data_mut(), params)?.read(request),))
    })
}
/// Installs one no-operand action whose native code may call back into this instance.
pub(crate) fn add_routed<T, E, P, V>(
    host: &mut LinkerInstance<'_, T>,
    name: &str,
    request: fn(Sender<Result<V, String>>) -> E::Request,
    endpoint: fn(&mut T, P) -> wasmtime::Result<E>,
) -> wasmtime::Result<()>
where
    T: WasiView + 'static,
    E: ReadEndpoint,
    P: ComponentNamedList + Lift + 'static,
    V: Into<crate::import_route::Value> + Send + 'static,
{
    host.func_wrap(name, move |mut store: StoreContextMut<'_, T>, params: P| {
        let route = endpoint(store.data_mut(), params)?.route(request);
        Ok((crate::import_route::push(store.data_mut(), route)?,))
    })
}
/// Installs one exact-operand action whose native code may call back into this instance.
pub(crate) fn add_routed_operand<T, E, P, A, V>(
    host: &mut LinkerInstance<'_, T>,
    name: &str,
    request: fn(A, Sender<Result<V, String>>) -> E::Request,
    endpoint: fn(&mut T, P) -> wasmtime::Result<(E, A)>,
) -> wasmtime::Result<()>
where
    T: WasiView + 'static,
    E: ReadEndpoint,
    P: ComponentNamedList + Lift + 'static,
    A: 'static,
    V: Into<crate::import_route::Value> + Send + 'static,
{
    host.func_wrap(name, move |mut store: StoreContextMut<'_, T>, params: P| {
        let (endpoint, value) = endpoint(store.data_mut(), params)?;
        let route = endpoint.route(|reply| request(value, reply));
        Ok((crate::import_route::push(store.data_mut(), route)?,))
    })
}
/// Installs one exact-operand read without changing absent or empty values.
pub(crate) fn add_operand<T, E, P, A, V>(
    host: &mut LinkerInstance<'_, T>,
    name: &str,
    request: fn(A, Sender<Result<V, String>>) -> E::Request,
    endpoint: fn(&mut T, P) -> wasmtime::Result<(E, A)>,
) -> wasmtime::Result<()>
where
    T: WasiView + 'static,
    E: ReadEndpoint,
    P: ComponentNamedList + Lift + 'static,
    A: 'static,
    V: ComponentType + Lower + Send + 'static,
{
    host.func_wrap(name, move |mut store: StoreContextMut<'_, T>, params: P| {
        let (endpoint, value) = endpoint(store.data_mut(), params)?;
        Ok((endpoint.read(|reply| request(value, reply)),))
    })
}

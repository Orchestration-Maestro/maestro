//! Canonical HTTP resources forward through the original local capability owner.
use crate::{
    api_imports::ApiView,
    context_imports::add_resource,
    local_resources::{
        Message, SignalEndpoint,
        http::{Fetch, Request, Response},
        native_resources::Reader,
    },
};
use wasmtime::{
    StoreContextMut,
    component::{ComponentType, Lift, LinkerInstance, Resource},
};

/// Canonical supplied HTTP operands, with body presence preserved.
#[derive(ComponentType, Lift)]
#[component(record)]
struct HttpRequest {
    /// Supplied URL.
    url: String,
    /// Omitted selects GET.
    method: Option<String>,
    /// Supplied header pairs.
    headers: Vec<(String, String)>,
    /// Absent and present-empty remain distinct.
    body: Option<Vec<u8>>,
}
/// Registers response ownership and one native attempt without JSON resource IDs.
pub(crate) fn add_to_linker<T: ApiView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    add_resource::<T, Response>(host, "response")?;
    host.func_wrap_concurrent(
        "fetch",
        |accessor, (request, signal): (HttpRequest, Option<Resource<SignalEndpoint>>)| {
            Box::pin(async move {
                let (api, signal) = accessor.with(|mut a| {
                    let state = a.get();
                    let signal = signal
                        .map(|signal| state.ctx().table.get(&signal).cloned())
                        .transpose()?;
                    Ok::<_, wasmtime::Error>((state.api().clone(), signal))
                })?;
                let fetch = Fetch {
                    url: request.url,
                    method: request.method,
                    headers: request.headers,
                    body: request.body,
                    signal,
                };
                let response = api
                    .0
                    .request_async(|id, reply, admission| {
                        Message::Http(id, Request::Fetch(fetch, reply, admission))
                    })
                    .await;
                let result = match response {
                    Ok(response) => Ok(accessor.with(|mut a| a.get().ctx().table.push(response))?),
                    Err(error) => Err(error),
                };
                Ok((result,))
            })
        },
    )?;
    text(host)?;
    metadata(host)?;
    host.func_wrap(
        "[method]response.status",
        |mut store: StoreContextMut<'_, T>, (response,): (Resource<Response>,)| {
            let response = store.data_mut().ctx().table.get(&response)?.clone();
            let result = response
                .0
                .request(|id, reply| Message::Http(id, Request::Status(reply)))
                .map_err(wasmtime::Error::msg)?;
            Ok((result,))
        },
    )
}

/// Projects native headers and body presence without collecting bytes.
fn metadata<T: ApiView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    host.func_wrap(
        "[method]response.status-text",
        |mut store: StoreContextMut<'_, T>, (response,): (Resource<Response>,)| {
            let response = store.data_mut().ctx().table.get(&response)?.clone();
            Ok((response
                .0
                .request(|id, reply| Message::Http(id, Request::StatusText(reply)))
                .map_err(wasmtime::Error::msg)?,))
        },
    )?;
    host.func_wrap(
        "[method]response.headers",
        |mut store: StoreContextMut<'_, T>, (response,): (Resource<Response>,)| {
            let response = store.data_mut().ctx().table.get(&response)?.clone();
            Ok((response
                .0
                .request(|id, reply| Message::Http(id, Request::Headers(reply)))
                .map_err(wasmtime::Error::msg)?,))
        },
    )?;
    host.func_wrap(
        "[method]response.body",
        |mut store: StoreContextMut<'_, T>, (response,): (Resource<Response>,)| {
            let response = store.data_mut().ctx().table.get(&response)?.clone();
            let body = response
                .0
                .request(|id, reply| Message::Http(id, Request::Body(reply)))
                .map_err(wasmtime::Error::msg)?;
            let body: Option<Resource<Reader>> = body
                .map(|body| store.data_mut().ctx().table.push(body))
                .transpose()?;
            Ok((body,))
        },
    )
}

/// Collects response text without holding a Store or capability-table borrow.
fn text<T: ApiView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    host.func_wrap_concurrent(
        "[method]response.text",
        |accessor, (response,): (Resource<Response>,)| {
            Box::pin(async move {
                let response =
                    accessor.with(|mut a| a.get().ctx().table.get(&response).cloned())?;
                let result = response
                    .0
                    .request_async(|id, reply, admission| {
                        Message::Http(id, Request::Text(reply, admission))
                    })
                    .await;
                Ok((result,))
            })
        },
    )
}

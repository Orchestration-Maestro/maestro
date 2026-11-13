//! Canonical HTTP resources forward through the original local capability owner.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{
    api_imports::ApiView,
    bindings::maestro::extension::host::HostResponse,
    context_imports::add_resource,
    lifecycle::State,
    local_resources::{
        Message, SignalEndpoint,
        http::{Fetch, Request, Response},
        native_resources::Reader,
    },
    read_imports::{endpoint, forwards, release},
};
use wasmtime::component::{ComponentType, Lift, LinkerInstance, Resource};
use wasmtime_wasi::WasiView;

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
/// Registers one native attempt and the response consumers without JSON resource IDs.
pub(crate) fn add_to_linker<T: ApiView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
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
    consumers(host)
}

/// Consumes a native response without holding a Store or capability-table borrow.
fn consumers<T: ApiView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    host.func_wrap_concurrent(
        "[method]response.text",
        |accessor, (response,): (Resource<Response>,)| {
            Box::pin(async move {
                let response =
                    accessor.with(|mut a| a.get().ctx().table.get(&response).cloned())?;
                Ok((response.text().await,))
            })
        },
    )?;
    host.func_wrap_concurrent(
        "[method]response.json",
        |accessor, (response,): (Resource<Response>,)| {
            Box::pin(async move {
                let response =
                    accessor.with(|mut a| a.get().ctx().table.get(&response).cloned())?;
                Ok((response.json().await,))
            })
        },
    )
}

impl HostResponse for State {
    fn status(&mut self, this: Resource<Response>) -> wasmtime::Result<u16> {
        endpoint(self, &this)?
            .status()
            .map_err(wasmtime::Error::msg)
    }

    fn status_text(&mut self, this: Resource<Response>) -> wasmtime::Result<String> {
        endpoint(self, &this)?
            .status_text()
            .map_err(wasmtime::Error::msg)
    }

    fn headers(&mut self, this: Resource<Response>) -> wasmtime::Result<Vec<(String, String)>> {
        endpoint(self, &this)?
            .headers()
            .map_err(wasmtime::Error::msg)
    }

    fn body(&mut self, this: Resource<Response>) -> wasmtime::Result<Option<Resource<Reader>>> {
        let body = endpoint(self, &this)?
            .body()
            .map_err(wasmtime::Error::msg)?;
        Ok(body.map(|body| self.ctx().table.push(body)).transpose()?)
    }

    fn drop(&mut self, this: Resource<Response>) -> wasmtime::Result<()> {
        release(self, this)
    }
}

/// Forwards the synchronous methods of a response to its trait implementation.
pub(crate) fn add_response(host: &mut LinkerInstance<'_, State>) -> wasmtime::Result<()> {
    add_resource::<State, Response>(host, "response")?;
    forwards! { host, "response", HostResponse, Response,
        "status" => status();
        "status-text" => status_text();
        "headers" => headers();
        "body" => body();
    }
    Ok(())
}

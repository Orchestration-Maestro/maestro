//! Supplied HTTP operands and response resources for author transport tests.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::host::{Imports, State};
use crate::bindings::host_side::maestro::extension::host;
use wasmtime::component::{Accessor, Resource};

/// Exact request operands observed before native normalization.
pub struct Request {
    /// Supplied URL.
    pub url: String,
    /// Supplied method, including omission.
    pub method: Option<String>,
    /// Supplied header pairs.
    pub headers: Vec<(String, String)>,
    /// Supplied body, including present-empty.
    pub body: Option<Vec<u8>>,
    /// Original supplied cancellation identity.
    pub signal: Option<usize>,
}
/// A controlled response, not a claim of native networking.
pub struct Response {
    /// Supplied status.
    pub status: u16,
    /// Supplied reason phrase.
    pub status_text: String,
    /// Supplied headers.
    pub headers: Vec<(String, String)>,
    /// Supplied consuming byte resource.
    pub body: Option<super::native_io_host::Reader>,
}
/// Supplied responses and exact request observations.
pub struct Bank {
    /// Supplied status.
    pub status: u16,
    /// Supplied reason phrase.
    pub status_text: String,
    /// Supplied normalized headers.
    pub headers: Vec<(String, String)>,
    /// Supplied body chunks, including an absent body.
    pub body: Option<Vec<Vec<u8>>>,
    /// Admitted requests in order.
    pub requests: Vec<Request>,
}
impl Default for Bank {
    fn default() -> Self {
        Self {
            status: 200,
            status_text: String::new(),
            headers: vec![],
            body: Some(vec![]),
            requests: vec![],
        }
    }
}
impl Bank {
    /// Records exact operands without interpreting them as native HTTP.
    pub fn fetch(&mut self, request: Request) -> Response {
        self.requests.push(request);
        Response {
            status: self.status,
            status_text: self.status_text.clone(),
            headers: self.headers.clone(),
            body: self.body.clone().map(super::native_io_host::Reader::new),
        }
    }
}
/// Supplies a response through the generated import path.
pub(super) fn fetch(
    accessor: &Accessor<State, Imports>,
    request: host::HttpRequest,
    signal: Option<Resource<super::host::Flag>>,
) -> wasmtime::Result<Result<Resource<Response>, String>> {
    accessor.with(|mut a| {
        let state = a.get();
        let signal = signal
            .map(|signal| {
                state
                    .table
                    .get(&signal)
                    .map(|flag| std::sync::Arc::as_ptr(&flag.0) as usize)
            })
            .transpose()?;
        let response = state.observed.native_http.fetch(Request {
            url: request.url,
            method: request.method,
            headers: request.headers,
            body: request.body,
            signal,
        });
        Ok(Ok(state.table.push(response)?))
    })
}
impl host::HostResponse for State {
    fn status_text(&mut self, response: Resource<Response>) -> wasmtime::Result<String> {
        Ok(self.table.get(&response)?.status_text.clone())
    }
    fn headers(&mut self, response: Resource<Response>) -> wasmtime::Result<Vec<(String, String)>> {
        Ok(self.table.get(&response)?.headers.clone())
    }
    fn body(
        &mut self,
        response: Resource<Response>,
    ) -> wasmtime::Result<Option<Resource<super::native_io_host::Reader>>> {
        self.table
            .get(&response)?
            .body
            .clone()
            .map(|body| self.table.push(body))
            .transpose()
            .map_err(Into::into)
    }
    fn status(&mut self, response: Resource<Response>) -> wasmtime::Result<u16> {
        Ok(self.table.get(&response)?.status)
    }
    fn drop(&mut self, response: Resource<Response>) -> wasmtime::Result<()> {
        self.table.delete(response)?;
        Ok(())
    }
}

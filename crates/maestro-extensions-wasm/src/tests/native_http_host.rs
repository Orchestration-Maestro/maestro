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
    /// Supplied decoded text from the controlled native owner.
    pub text: String,
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
    /// Supplied decoded text from the controlled native owner.
    pub text: String,
    /// Supplied status.
    pub status: u16,
    /// Supplied reason phrase.
    pub status_text: String,
    /// Supplied normalized headers.
    pub headers: Vec<(String, String)>,
    /// Supplied body chunks, including an absent body.
    pub body: Option<Vec<Vec<u8>>>,
    /// Supplied failure before any response.
    pub error: Option<String>,
    /// Supplied failure after the body prefix.
    pub read_error: Option<String>,
    /// Admitted requests in order.
    pub requests: Vec<Request>,
}
impl Default for Bank {
    fn default() -> Self {
        Self {
            text: String::new(),
            status: 200,
            status_text: String::new(),
            headers: vec![],
            body: Some(vec![]),
            error: None,
            read_error: None,
            requests: vec![],
        }
    }
}
impl Bank {
    /// Records exact operands without interpreting them as native HTTP.
    pub fn fetch(&mut self, request: Request) -> Result<Response, String> {
        self.requests.push(request);
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        let body = self.body.clone().map(super::native_io_host::Reader::new);
        if let (Some(body), Some(error)) = (&body, &self.read_error) {
            body.fail(error.clone());
        }
        Ok(Response {
            text: self.text.clone(),
            status: self.status,
            status_text: self.status_text.clone(),
            headers: self.headers.clone(),
            body,
        })
    }
}
impl Response {
    /// Supplies a native JSON result with the same consuming-body admission.
    pub fn json(&self) -> Result<serde_json::Value, String> {
        serde_json::from_str(&self.text()?).map_err(|error| error.to_string())
    }
    /// Returns the supplied native text, enforcing shared one-time consumption.
    pub fn text(&self) -> Result<String, String> {
        if let Some(body) = &self.body {
            if !body.claim() {
                return Err("body already used".into());
            }
            body.close();
        }
        Ok(self.text.clone())
    }
}
impl host::HostResponseWithStore<State> for Imports {
    fn json(
        accessor: &Accessor<State, Self>,
        response: Resource<Response>,
    ) -> impl Future<Output = wasmtime::Result<Result<String, String>>> {
        std::future::ready(accessor.with(|mut a| {
            Ok(a.get()
                .table
                .get(&response)?
                .json()
                .and_then(|value| serde_json::to_string(&value).map_err(|error| error.to_string())))
        }))
    }
    fn text(
        accessor: &Accessor<State, Self>,
        response: Resource<Response>,
    ) -> impl Future<Output = wasmtime::Result<Result<String, String>>> {
        std::future::ready(accessor.with(|mut a| Ok(a.get().table.get(&response)?.text())))
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
        match response {
            Ok(response) => Ok(Ok(state.table.push(response)?)),
            Err(error) => Ok(Err(error)),
        }
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

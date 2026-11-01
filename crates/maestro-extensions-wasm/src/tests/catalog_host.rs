//! Controlled live context replies shared by both hosts.
use super::reader_host::{Data as Replies, Reader};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
/// Supplied usage, without integer conversion.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Usage {
    /// Nullable estimate.
    pub tokens: Option<f64>,
    /// Supplied capacity.
    pub context_window: f64,
    /// Nullable percentage.
    pub percent: Option<f64>,
}
/// Live state forwarded by context getters.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, default, rename_all = "camelCase")]
pub struct Data {
    /// Availability, independent of the opaque UI resource.
    #[serde(rename = "hasUI")]
    pub has_ui: bool,
    /// Idle flag.
    pub idle: bool,
    /// Queued-message flag, independent of idle.
    pub queued: bool,
    /// Literal prompt.
    pub prompt: String,
    /// Optional usage.
    pub usage: Option<Usage>,
    /// Optional current signal.
    pub signal: Option<bool>,
    /// Injected recoverable getter error.
    pub error: Option<String>,
}
impl Default for Data {
    fn default() -> Self {
        Self {
            has_ui: false,
            idle: true,
            queued: false,
            prompt: String::new(),
            usage: None,
            signal: None,
            error: None,
        }
    }
}
/// Synchronously acknowledged controls.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Control {
    /// Replace scalar state.
    state: Option<Data>,
    /// Bind an independently retained catalog.
    registry: Option<Replies>,
    /// Edit the existing retained catalog.
    registry_update: Option<Replies>,
    /// Bind a new model supplier.
    model: Option<Replies>,
    /// Edit the captured supplier's state.
    model_update: Option<Replies>,
    /// Reject future context calls until reset.
    stale: Option<String>,
    /// Cancel the signal captured before replacement.
    cancel_old: Option<bool>,
    /// Clear injected staleness.
    reset: Option<bool>,
}
/// Host binding; resources taken from it remain independent.
#[derive(Default)]
pub struct Bank {
    /// Live scalar getter state.
    pub data: Data,
    /// Current model supplier, captured by context creation.
    pub model: Reader,
    /// Current catalog.
    pub registry: Reader,
    /// Current signal owner.
    pub flag: Option<Arc<AtomicBool>>,
    /// Previous signal for cancellation witnesses.
    old_flag: Option<Arc<AtomicBool>>,
    /// First acquisition failure.
    stale: Option<String>,
}
impl Bank {
    /// Mutate the host before acknowledging the public entry operation.
    pub fn control(&mut self, text: &str) {
        let c: Control = serde_json::from_str(text).unwrap();
        if c.reset == Some(true) {
            self.stale = None;
        }
        if let Some(state) = c.state {
            self.old_flag = self.flag.take();
            self.flag = state.signal.map(|v| Arc::new(AtomicBool::new(v)));
            self.data = state;
        }
        if let Some(data) = c.registry {
            self.registry = Reader::from_data(data);
        }
        if let Some(data) = c.registry_update {
            *self.registry.0.lock().unwrap() = data;
        }
        if let Some(data) = c.model {
            self.model = Reader::from_data(data);
        }
        if let Some(data) = c.model_update {
            *self.model.0.lock().unwrap() = data;
        }
        if self.stale.is_none() {
            self.stale = c.stale;
        }
        if c.cancel_old == Some(true) {
            self.old_flag
                .as_ref()
                .unwrap()
                .store(true, Ordering::SeqCst);
        }
    }
    /// Check context acquisition, never retained resource queries.
    pub fn check(&self) -> Result<(), String> {
        self.stale
            .as_ref()
            .or(self.data.error.as_ref())
            .map_or(Ok(()), |e| Err(e.clone()))
    }
    /// Read the optional captured model result.
    pub fn selected_model(&self, reader: &Reader) -> Result<Option<String>, String> {
        self.check()?;
        if reader.0.lock().unwrap().replies.is_empty() {
            return Ok(None);
        }
        reader.record("model", &[])
    }
}
/// Authentication outcome controlled by supplied replies.
pub fn auth(reader: &Reader, model: &str) -> Result<Value, String> {
    let model: Value = serde_json::from_str(model).map_err(|e| e.to_string())?;
    let reply = reader.reply("auth", std::slice::from_ref(&model))?;
    let keys = |value: &Value| {
        value["headers"]
            .as_object()
            .map(|headers| headers.keys().cloned().collect::<Vec<_>>())
    };
    assert!(
        keys(&model) == keys(&reply.args[0]),
        "auth model header key order"
    );
    if let (Some(actual), Some(expected)) = (
        model["cost"]["input"].as_f64(),
        reply.args[0]["cost"]["input"].as_f64(),
    ) {
        assert!(
            actual.to_bits() == expected.to_bits(),
            "auth model numeric bits"
        );
    }
    Ok(reply.value)
}
/// Ordered authentication headers and optional key.
pub fn resolved(value: &Value) -> maestro_request::model_registry::ResolvedRequestAuth {
    if let Some(error) = value.get("error") {
        return crate::ResolvedRequestAuth::Error {
            error: error.as_str().unwrap().into(),
        };
    }
    crate::ResolvedRequestAuth::Ok {
        api_key: value["apiKey"].as_str().map(str::to_owned),
        headers: value["headers"].as_array().map(|pairs| {
            pairs
                .iter()
                .map(|p| (p[0].as_str().unwrap().into(), p[1].as_str().unwrap().into()))
                .collect()
        }),
        base_url: None,
    }
}

use super::host::{Imports, State};
use crate::bindings::host_side::maestro::extension::host;
use wasmtime::component::{Accessor, Resource};
impl host::HostModelRegistry for State {
    fn auth_storage(
        &mut self,
        this: Resource<Reader>,
    ) -> wasmtime::Result<Result<Resource<Reader>, String>> {
        let reader = self.table.get(&this)?.clone();
        Ok(Ok(self.push(reader)?))
    }

    fn get_all(&mut self, this: Resource<Reader>) -> wasmtime::Result<Result<String, String>> {
        Ok(self
            .table
            .get(&this)?
            .record("getAll", &[])
            .and_then(|v| v.ok_or("missing list".into())))
    }
    fn get_available(
        &mut self,
        this: Resource<Reader>,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        let result = self
            .table
            .get(&this)?
            .record("getAvailable", &[])
            .and_then(|v| v.ok_or("missing list".into()))
            .map(host::RouteValue::Text);
        self.push(super::host::Route {
            steps: std::collections::VecDeque::new(),
            running: None,
            rows: None,
            result: Some(result),
        })
    }
    fn find(
        &mut self,
        this: Resource<Reader>,
        provider: String,
        model_id: String,
    ) -> wasmtime::Result<Result<Option<String>, String>> {
        Ok(self
            .table
            .get(&this)?
            .record("find", &[json!(provider), json!(model_id)]))
    }
    fn refresh(
        &mut self,
        this: Resource<Reader>,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        let result = self.table.get(&this)?.unit("refresh", &[]);
        self.routed(result)
    }
    fn get_error(
        &mut self,
        this: Resource<Reader>,
    ) -> wasmtime::Result<Result<Option<String>, String>> {
        Ok(self.table.get(&this)?.optional("getError", &[]))
    }
    fn has_configured_auth(
        &mut self,
        this: Resource<Reader>,
        model: String,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        let result = self
            .table
            .get(&this)?
            .flag("hasConfiguredAuth", &[json!(model)])
            .map(host::RouteValue::Boolean);
        self.push(super::host::Route {
            steps: std::collections::VecDeque::new(),
            running: None,
            rows: None,
            result: Some(result),
        })
    }
    fn get_provider_auth_status(
        &mut self,
        this: Resource<Reader>,
        provider: String,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        let result = self
            .table
            .get(&this)?
            .record("getProviderAuthStatus", &[json!(provider)])
            .and_then(|v| v.ok_or("missing status".into()))
            .map(host::RouteValue::Text);
        self.push(super::host::Route {
            steps: std::collections::VecDeque::new(),
            running: None,
            rows: None,
            result: Some(result),
        })
    }
    fn get_provider_display_name(
        &mut self,
        this: Resource<Reader>,
        provider: String,
    ) -> wasmtime::Result<Result<String, String>> {
        Ok(self
            .table
            .get(&this)?
            .optional("getProviderDisplayName", &[json!(provider)])
            .and_then(|v| v.ok_or("missing name".into())))
    }
    fn is_using_oauth(
        &mut self,
        this: Resource<Reader>,
        model: String,
    ) -> wasmtime::Result<Result<bool, String>> {
        Ok(self.table.get(&this)?.flag("isUsingOAuth", &[json!(model)]))
    }
    fn register_provider(
        &mut self,
        this: Resource<Reader>,
        name: String,
        config: String,
        _callbacks: (
            Option<Resource<super::host::Identity>>,
            Option<(Resource<super::host::Identity>, String, Option<bool>)>,
        ),
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        let result = self
            .table
            .get(&this)?
            .unit("registerProvider", &[json!(name), json!(config)]);
        self.routed(result)
    }
    fn unregister_provider(
        &mut self,
        this: Resource<Reader>,
        name: String,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        let result = self
            .table
            .get(&this)?
            .unit("unregisterProvider", &[json!(name)]);
        self.routed(result)
    }
    fn drop(&mut self, this: Resource<Reader>) -> wasmtime::Result<()> {
        self.release(this)
    }
}
impl host::HostModelRegistryWithStore<State> for Imports {
    async fn get_api_key_for_provider(
        accessor: &Accessor<State, Self>,
        this: Resource<Reader>,
        provider: String,
    ) -> wasmtime::Result<Result<Option<String>, String>> {
        let hold = accessor.with(|mut a| a.get().observed.take_hold());
        super::observed::pause(hold).await;
        accessor.with(|mut a| {
            Ok(a.get()
                .table
                .get(&this)?
                .optional("getApiKeyForProvider", &[json!(provider)]))
        })
    }
    async fn get_api_key_and_headers(
        accessor: &Accessor<State, Self>,
        this: Resource<Reader>,
        model: String,
    ) -> wasmtime::Result<Result<host::ResolvedRequestAuth, String>> {
        let hold = accessor.with(|mut a| {
            let o = &mut a.get().observed;
            o.log("auth started");
            o.take_hold()
        });
        super::observed::pause(hold).await;
        accessor.with(|mut a| {
            let s = a.get();
            let answer = auth(s.table.get(&this)?, &model).map(|v| match resolved(&v) {
                crate::ResolvedRequestAuth::Ok {
                    api_key, headers, ..
                } => host::ResolvedRequestAuth::Ok(host::RequestAuth {
                    base_url: None,
                    api_key,
                    headers: headers.map(|h| h.into_iter().collect()),
                }),
                crate::ResolvedRequestAuth::Error { error } => {
                    host::ResolvedRequestAuth::Error(error)
                }
            });
            s.observed.log("auth completed");
            Ok(answer)
        })
    }
}

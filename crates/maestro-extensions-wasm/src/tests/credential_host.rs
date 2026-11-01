//! Credential wire replies through the existing controlled reader resource.
use super::{
    host::{Imports, State},
    reader_host::Reader,
};
use crate::bindings::host_side::maestro::extension::host;
use serde_json::json;
use wasmtime::component::{Accessor, Resource};
impl host::HostAuthStorage for State {
    fn get_oauth_providers(
        &mut self,
        this: Resource<Reader>,
    ) -> wasmtime::Result<Result<Vec<Resource<Reader>>, String>> {
        let result = self
            .table
            .get(&this)?
            .oauth_providers()
            .and_then(|providers| {
                providers
                    .into_iter()
                    .map(|provider| self.push(provider).map_err(|error| error.to_string()))
                    .collect()
            });
        Ok(result)
    }
    fn set_fallback_resolver(
        &mut self,
        this: Resource<Reader>,
        resolver: Resource<super::host::Identity>,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        let reader = self.table.get(&this)?.clone();
        let result = reader.unit("set_fallback_resolver", &[]);
        if result.is_ok() {
            reader.1.lock().unwrap().fallback = Some(resolver.rep());
            self.observed
                .register("credential-fallback".into(), resolver.rep());
        }
        self.routed(result)
    }
    fn set_runtime_api_key(
        &mut self,
        this: Resource<Reader>,
        provider: String,
        key: String,
    ) -> wasmtime::Result<Result<(), String>> {
        Ok(self
            .table
            .get(&this)?
            .unit("set_runtime_api_key", &[json!(provider), json!(key)]))
    }
    fn remove_runtime_api_key(
        &mut self,
        this: Resource<Reader>,
        provider: String,
    ) -> wasmtime::Result<Result<(), String>> {
        Ok(self
            .table
            .get(&this)?
            .unit("remove_runtime_api_key", &[json!(provider)]))
    }
    fn reload(&mut self, this: Resource<Reader>) -> wasmtime::Result<Resource<super::host::Route>> {
        let reader = self.table.get(&this)?.clone();
        let result = reader.unit("reload", &[]).map(|()| host::RouteValue::Unit);
        self.credential_backend_route(&reader, result)
    }
    fn get(
        &mut self,
        this: Resource<Reader>,
        provider: String,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        let result = self.table.get(&this)?.record("get", &[json!(provider)]);
        self.routed_value(result.map(host::RouteValue::OptionalText))
    }
    fn set(
        &mut self,
        this: Resource<Reader>,
        provider: String,
        credential: String,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        let result = self
            .table
            .get(&this)?
            .unit("set", &[json!(provider), json!(credential)]);
        let reader = self.table.get(&this)?.clone();
        self.credential_backend_route(&reader, result.map(|()| host::RouteValue::Unit))
    }
    fn remove(
        &mut self,
        this: Resource<Reader>,
        provider: String,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        let result = self.table.get(&this)?.unit("remove", &[json!(provider)]);
        self.routed(result)
    }
    fn list(&mut self, this: Resource<Reader>) -> wasmtime::Result<Resource<super::host::Route>> {
        let result = self
            .table
            .get(&this)?
            .record("list", &[])
            .and_then(|value| {
                serde_json::from_str(&value.ok_or("missing credential list")?)
                    .map_err(|error| error.to_string())
            });
        self.routed_value(result.map(host::RouteValue::Lines))
    }
    fn has(
        &mut self,
        this: Resource<Reader>,
        provider: String,
    ) -> wasmtime::Result<Result<bool, String>> {
        Ok(self.table.get(&this)?.flag("has", &[json!(provider)]))
    }
    fn has_auth(
        &mut self,
        this: Resource<Reader>,
        provider: String,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        let reader = self.table.get(&this)?.clone();
        let result = reader
            .flag("has_auth", &[json!(provider)])
            .map(host::RouteValue::Boolean);
        self.credential_fallback_route(&reader, provider, result)
    }
    fn get_auth_status(
        &mut self,
        this: Resource<Reader>,
        provider: String,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        let reader = self.table.get(&this)?.clone();
        let result = reader
            .record("get_auth_status", &[json!(provider)])
            .and_then(|value| value.ok_or("missing credential record".into()))
            .map(host::RouteValue::Text);
        self.credential_fallback_route(&reader, provider, result)
    }
    fn get_all(
        &mut self,
        this: Resource<Reader>,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        let result = self
            .table
            .get(&this)?
            .record("get_all", &[])
            .and_then(|value| value.ok_or("missing credential record".into()));
        self.routed_value(result.map(host::RouteValue::Text))
    }
    fn drain_errors(&mut self, this: Resource<Reader>) -> wasmtime::Result<Result<String, String>> {
        Ok(self
            .table
            .get(&this)?
            .record("drain_errors", &[])
            .and_then(|value| value.ok_or("missing credential record".into())))
    }
    fn logout(
        &mut self,
        this: Resource<Reader>,
        provider: String,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        let result = self.table.get(&this)?.unit("logout", &[json!(provider)]);
        self.routed(result)
    }
    fn drop(&mut self, this: Resource<Reader>) -> wasmtime::Result<()> {
        self.release(this)
    }
}
impl host::HostAuthStorageWithStore<State> for Imports {
    fn login(
        accessor: &Accessor<State, Self>,
        this: Resource<Reader>,
        provider: String,
        _callbacks: Resource<super::host::Identity>,
        _options: (bool, bool, bool),
        _signal: Option<Resource<super::host::Flag>>,
    ) -> impl Future<Output = wasmtime::Result<Result<(), String>>> {
        std::future::ready(accessor.with(|mut access| {
            Ok(access
                .get()
                .table
                .get(&this)?
                .unit("login", &[json!(provider)]))
        }))
    }
    async fn get_api_key(
        accessor: &Accessor<State, Self>,
        this: Resource<Reader>,
        provider: String,
        include_fallback: Option<bool>,
    ) -> wasmtime::Result<Result<Option<String>, String>> {
        let (reader, guest, hold) = accessor.with(|mut access| {
            let state = access.get();
            Ok::<_, wasmtime::Error>((
                state.table.get(&this)?.clone(),
                state.guest.clone().unwrap(),
                state.observed.take_hold(),
            ))
        })?;
        super::observed::pause(hold).await;
        if include_fallback != Some(false) && scripted_fallback(&reader, &provider) {
            let target = reader.1.lock().unwrap().fallback.unwrap();
            let actual = guest
                .call_invoke_auth_fallback(accessor, Resource::new_borrow(target), provider.clone())
                .await?;
            let expected = reader.optional("fallback_result", &[json!(provider)]);
            assert_eq!(actual, expected);
            if let Err(error) = actual {
                return Ok(Err(error));
            }
        }
        Ok(reader.optional("get_api_key", &[json!(provider), json!(include_fallback)]))
    }
}

impl host::HostAuthLockUpdate for State {
    fn update(
        &mut self,
        this: Resource<Reader>,
        current: Option<String>,
    ) -> wasmtime::Result<Result<Option<String>, String>> {
        Ok(self
            .table
            .get(&this)?
            .optional("lock_update", &[json!(current)]))
    }
    fn drop(&mut self, this: Resource<Reader>) -> wasmtime::Result<()> {
        self.release(this)
    }
}
impl host::HostAuthLockUpdateWithStore<State> for Imports {
    fn update_async(
        accessor: &Accessor<State, Self>,
        this: Resource<Reader>,
        current: Option<String>,
    ) -> impl Future<Output = wasmtime::Result<Result<Option<String>, String>>> {
        std::future::ready(accessor.with(|mut access| {
            Ok(access
                .get()
                .table
                .get(&this)?
                .optional("lock_update_async", &[json!(current)]))
        }))
    }
}

impl host::HostOauthProvider for State {
    fn modify_models(
        &mut self,
        this: Resource<Reader>,
        data: String,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        let result = self
            .table
            .get(&this)?
            .optional("oauth_models", &[json!(data)])
            .and_then(|value| value.ok_or_else(|| "missing OAuth model result".into()));
        self.routed_value(result.map(host::RouteValue::Text))
    }

    fn info(&mut self, this: Resource<Reader>) -> wasmtime::Result<Result<String, String>> {
        Ok(self.table.get(&this)?.string("oauth_info"))
    }
    fn get_api_key(
        &mut self,
        this: Resource<Reader>,
        credentials: String,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        let result = self
            .table
            .get(&this)?
            .optional("oauth_key", &[json!(credentials)])
            .and_then(|value| value.ok_or_else(|| "missing OAuth key".into()));
        self.routed_value(result.map(host::RouteValue::Text))
    }
    fn drop(&mut self, this: Resource<Reader>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

impl host::HostOauthProviderWithStore<State> for Imports {
    fn login(
        accessor: &Accessor<State, Self>,
        this: Resource<Reader>,
        _callbacks: Resource<super::host::Identity>,
        _options: (bool, bool, bool),
        _signal: Option<Resource<super::host::Flag>>,
    ) -> impl Future<Output = wasmtime::Result<Result<String, String>>> {
        std::future::ready(accessor.with(|mut access| {
            Ok(access
                .get()
                .table
                .get(&this)?
                .optional("oauth_login", &[])
                .and_then(|value| value.ok_or_else(|| "missing OAuth login result".into())))
        }))
    }
    fn refresh_token(
        accessor: &Accessor<State, Self>,
        this: Resource<Reader>,
        credentials: String,
    ) -> impl Future<Output = wasmtime::Result<Result<String, String>>> {
        std::future::ready(accessor.with(|mut access| {
            Ok(access
                .get()
                .table
                .get(&this)?
                .optional("oauth_refresh", &[json!(credentials)])
                .and_then(|value| value.ok_or_else(|| "missing OAuth refresh result".into())))
        }))
    }
}

impl host::HostOauthInteraction for State {
    fn options(&mut self, _this: Resource<Reader>) -> wasmtime::Result<(bool, bool, bool)> {
        Err(wasmtime::format_err!(
            "unexpected controlled OAuth interaction"
        ))
    }
    fn signal(
        &mut self,
        _this: Resource<Reader>,
    ) -> wasmtime::Result<Option<Resource<super::host::Flag>>> {
        Err(wasmtime::format_err!(
            "unexpected controlled OAuth interaction"
        ))
    }
    fn notify(
        &mut self,
        _this: Resource<Reader>,
        _phase: host::OauthCallback,
        _data: String,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        Err(wasmtime::format_err!(
            "unexpected controlled OAuth interaction"
        ))
    }
    fn drop(&mut self, this: Resource<Reader>) -> wasmtime::Result<()> {
        self.release(this)
    }
}
impl host::HostOauthInteractionWithStore<State> for Imports {
    fn input(
        _accessor: &Accessor<State, Self>,
        _this: Resource<Reader>,
        _phase: host::OauthCallback,
        _data: String,
    ) -> impl Future<Output = wasmtime::Result<Result<Option<String>, String>>> {
        std::future::ready(Err(wasmtime::format_err!(
            "unexpected controlled OAuth interaction"
        )))
    }
}

/// The script, not a duplicated key-selection algorithm, chooses callback entry.
pub fn scripted_fallback(reader: &Reader, provider: &str) -> bool {
    reader.1.lock().unwrap().fallback.is_some()
        && reader
            .0
            .lock()
            .unwrap()
            .replies
            .iter()
            .any(|reply| reply.method == "fallback_result" && reply.args == [json!(provider)])
}

impl State {
    /// Runs the authored backend on the existing value-returning continuation.
    pub(super) fn credential_backend_route(
        &mut self,
        reader: &Reader,
        result: Result<host::RouteValue, String>,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        let mut steps = std::collections::VecDeque::new();
        let target = reader.1.lock().unwrap().backend;
        if let Some(target) = target {
            let expected = reader
                .optional("backend_result", &[])
                .map_err(wasmtime::Error::msg)?;
            let update = self.push(reader.clone())?;
            steps.push_back((
                host::LocalCall::AuthBackend(host::LocalAuthBackend {
                    target,
                    update,
                    asynchronous: false,
                }),
                super::ui_host::Nested::CredentialBackend(expected),
            ));
        }
        self.push(super::host::Route {
            steps,
            running: None,
            rows: None,
            result: Some(result),
        })
    }
    /// Requires the callback's actual non-unit result before publishing scripted metadata.
    fn credential_fallback_route(
        &mut self,
        reader: &Reader,
        provider: String,
        result: Result<host::RouteValue, String>,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        let mut steps = std::collections::VecDeque::new();
        if scripted_fallback(reader, &provider) {
            let resource = reader.1.lock().unwrap().fallback.unwrap();
            let target = self
                .table
                .get(&Resource::<super::host::Identity>::new_borrow(resource))?
                .0;
            let expected = reader.optional("fallback_result", &[json!(provider)]);
            steps.push_back((
                host::LocalCall::AuthFallback(host::LocalAuthFallback { target, provider }),
                super::ui_host::Nested::CredentialFallback(expected),
            ));
        }
        self.push(super::host::Route {
            steps,
            running: None,
            rows: None,
            result: Some(result),
        })
    }
}

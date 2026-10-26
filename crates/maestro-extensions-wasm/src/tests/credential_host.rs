//! Credential wire replies through the existing controlled reader resource.
use super::{
    host::{Imports, State},
    reader_host::Reader,
};
use crate::bindings::host_side::maestro::extension::host;
use serde_json::json;
use wasmtime::component::{Accessor, Resource};
impl host::HostAuthStorage for State {
    fn set_fallback_resolver(
        &mut self,
        this: Resource<Reader>,
        _resolver: Resource<super::host::Identity>,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        let result = self.table.get(&this)?.unit("set_fallback_resolver", &[]);
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
        let result = self.table.get(&this)?.unit("reload", &[]);
        self.routed(result)
    }
    fn get(
        &mut self,
        this: Resource<Reader>,
        provider: String,
    ) -> wasmtime::Result<Result<Option<String>, String>> {
        Ok(self.table.get(&this)?.record("get", &[json!(provider)]))
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
        self.routed(result)
    }
    fn remove(
        &mut self,
        this: Resource<Reader>,
        provider: String,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        let result = self.table.get(&this)?.unit("remove", &[json!(provider)]);
        self.routed(result)
    }
    fn list(&mut self, this: Resource<Reader>) -> wasmtime::Result<Result<Vec<String>, String>> {
        Ok(self
            .table
            .get(&this)?
            .record("list", &[])
            .and_then(|value| {
                serde_json::from_str(&value.ok_or("missing credential list")?)
                    .map_err(|error| error.to_string())
            }))
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
        let result = self
            .table
            .get(&this)?
            .flag("has_auth", &[json!(provider)])
            .map(host::RouteValue::Boolean);
        self.push(super::host::Route(Some(result)))
    }
    fn get_auth_status(
        &mut self,
        this: Resource<Reader>,
        provider: String,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        let result = self
            .table
            .get(&this)?
            .record("get_auth_status", &[json!(provider)])
            .and_then(|value| value.ok_or("missing credential record".into()))
            .map(host::RouteValue::Text);
        self.push(super::host::Route(Some(result)))
    }
    fn get_all(&mut self, this: Resource<Reader>) -> wasmtime::Result<Result<String, String>> {
        Ok(self
            .table
            .get(&this)?
            .record("get_all", &[])
            .and_then(|value| value.ok_or("missing credential record".into())))
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
    fn get_api_key(
        accessor: &Accessor<State, Self>,
        this: Resource<Reader>,
        provider: String,
        include_fallback: Option<bool>,
    ) -> impl Future<Output = wasmtime::Result<Result<Option<String>, String>>> {
        std::future::ready(accessor.with(|mut access| {
            Ok(access
                .get()
                .table
                .get(&this)?
                .optional("get_api_key", &[json!(provider), json!(include_fallback)]))
        }))
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

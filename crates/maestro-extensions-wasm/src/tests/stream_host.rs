//! The author-only harness rejects stream construction; stream witnesses use the production host.
use super::host::{Identity, Imports, Route, State};
use crate::bindings::host_side::maestro::extension::host;
use wasmtime::component::{Accessor, Resource};

/// These operations are deliberately exercised only through the real native adapter.
pub(super) const PRODUCTION_HOST: &str = "authored streams require the production host";
/// No author-only test can construct a native stream.
pub enum Stream {}
/// No author-only test can construct a retained stream value.
pub enum Value {}

impl host::HostStreamValue for State {
    fn new(&mut self, _: Resource<Identity>) -> wasmtime::Result<Resource<Value>> {
        Err(wasmtime::Error::msg(PRODUCTION_HOST))
    }
    fn id(&mut self, value: Resource<Value>) -> wasmtime::Result<u32> {
        match *self.table.get(&value)? {}
    }
    fn drop(&mut self, value: Resource<Value>) -> wasmtime::Result<()> {
        match self.table.delete(value)? {}
    }
}
impl host::HostEventStream for State {
    fn push(
        &mut self,
        stream: Resource<Stream>,
        _: Resource<Value>,
    ) -> wasmtime::Result<Resource<Route>> {
        match *self.table.get(&stream)? {}
    }
    fn end(
        &mut self,
        stream: Resource<Stream>,
        _: Option<Resource<Value>>,
    ) -> wasmtime::Result<Resource<Route>> {
        match *self.table.get(&stream)? {}
    }
    fn drop(&mut self, stream: Resource<Stream>) -> wasmtime::Result<()> {
        match self.table.delete(stream)? {}
    }
}
impl host::HostEventStreamWithStore<State> for Imports {
    fn next(
        a: &Accessor<State, Self>,
        stream: Resource<Stream>,
    ) -> impl std::future::Future<Output = wasmtime::Result<Result<Option<Resource<Value>>, String>>>
    {
        std::future::ready(a.with(|mut access| match *access.get().table.get(&stream)? {}))
    }
    fn result(
        a: &Accessor<State, Self>,
        stream: Resource<Stream>,
    ) -> impl std::future::Future<Output = wasmtime::Result<Result<Resource<Value>, String>>> {
        std::future::ready(a.with(|mut access| match *access.get().table.get(&stream)? {}))
    }
}

/// Model invocation is exercised only by the real native adapter.
pub enum Assistant {}
impl host::HostAssistantStream for State {
    fn push(
        &mut self,
        stream: Resource<Assistant>,
        _: String,
    ) -> wasmtime::Result<Result<(), String>> {
        match *self.table.get(&stream)? {}
    }
    fn end(
        &mut self,
        stream: Resource<Assistant>,
        _: Option<String>,
    ) -> wasmtime::Result<Result<(), String>> {
        match *self.table.get(&stream)? {}
    }
    fn drop(&mut self, stream: Resource<Assistant>) -> wasmtime::Result<()> {
        match self.table.delete(stream)? {}
    }
}
impl host::HostAssistantStreamWithStore<State> for Imports {
    fn next(
        a: &Accessor<State, Self>,
        stream: Resource<Assistant>,
    ) -> impl std::future::Future<Output = wasmtime::Result<Result<Option<String>, String>>> {
        std::future::ready(a.with(|mut access| match *access.get().table.get(&stream)? {}))
    }
    fn result(
        a: &Accessor<State, Self>,
        stream: Resource<Assistant>,
    ) -> impl std::future::Future<Output = wasmtime::Result<Result<String, String>>> {
        std::future::ready(a.with(|mut access| match *access.get().table.get(&stream)? {}))
    }
}

/// Native hooks cannot be constructed by the author-only harness.
pub enum Controls {}
impl host::HostProviderControls for State {
    fn drop(&mut self, controls: Resource<Controls>) -> wasmtime::Result<()> {
        match self.table.delete(controls)? {}
    }
}
impl host::HostProviderControlsWithStore<State> for Imports {
    fn invoke(
        a: &Accessor<State, Self>,
        controls: Resource<Controls>,
        _: host::ModelHook,
        _: String,
        _: String,
        _: Option<Resource<super::host::Flag>>,
        _: Option<Resource<Identity>>,
    ) -> impl std::future::Future<Output = wasmtime::Result<Result<host::PayloadSelection, String>>>
    {
        std::future::ready(a.with(|mut access| match *access.get().table.get(&controls)? {}))
    }
}

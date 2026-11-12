//! Canonical progress and subscription operations retain their native owners.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{
    bindings::maestro::extension::host::{
        HostBashOutput, HostSubscription, HostToolInvalidator, HostToolUpdate,
    },
    context_imports::add_resource,
    import_route::{Route, push},
    lifecycle::State,
    local_resources::{
        bash_output::Endpoint as BashOutput,
        effects::{
            EmitterEndpoint, InvalidatorEndpoint, SubscriptionEndpoint, ThemeEndpoint, ToolUpdate,
        },
    },
    read_imports::{endpoint, forwards, release},
};
use wasmtime::component::{LinkerInstance, Resource};

impl HostBashOutput for State {
    fn on_data(
        &mut self,
        this: Resource<BashOutput>,
        bytes: Vec<u8>,
    ) -> wasmtime::Result<Result<(), String>> {
        Ok(endpoint(self, &this)?.on_data(bytes))
    }

    fn drop(&mut self, this: Resource<BashOutput>) -> wasmtime::Result<()> {
        release(self, this)
    }
}

impl HostToolUpdate for State {
    fn update(
        &mut self,
        this: Resource<ToolUpdate>,
        partial: String,
    ) -> wasmtime::Result<Resource<Route>> {
        let route = endpoint(self, &this)?.update(partial);
        push(self, route)
    }

    fn drop(&mut self, this: Resource<ToolUpdate>) -> wasmtime::Result<()> {
        release(self, this)
    }
}

impl HostSubscription for State {
    fn unsubscribe(
        &mut self,
        this: Resource<SubscriptionEndpoint>,
    ) -> wasmtime::Result<Result<(), String>> {
        Ok(endpoint(self, &this)?.unsubscribe())
    }

    fn drop(&mut self, this: Resource<SubscriptionEndpoint>) -> wasmtime::Result<()> {
        release(self, this)
    }
}

impl HostToolInvalidator for State {
    fn invalidate(
        &mut self,
        this: Resource<InvalidatorEndpoint>,
    ) -> wasmtime::Result<Result<(), String>> {
        Ok(endpoint(self, &this)?.invalidate())
    }

    fn drop(&mut self, this: Resource<InvalidatorEndpoint>) -> wasmtime::Result<()> {
        release(self, this)
    }
}

/// Installs real operand resources without constructing a substitute observer or bus.
pub(crate) fn add_to_linker(host: &mut LinkerInstance<'_, State>) -> wasmtime::Result<()> {
    add_resource::<State, BashOutput>(host, "bash-output")?;
    add_resource::<State, ToolUpdate>(host, "tool-update")?;
    add_resource::<State, SubscriptionEndpoint>(host, "subscription")?;
    add_resource::<State, ThemeEndpoint>(host, "theme")?;
    add_resource::<State, EmitterEndpoint>(host, "callback-emitter")?;
    add_resource::<State, InvalidatorEndpoint>(host, "tool-invalidator")?;
    forwards! { host, "tool-update", HostToolUpdate, ToolUpdate,
        "update" => update(partial: String);
    }
    forwards! { host, "subscription", HostSubscription, SubscriptionEndpoint,
        "unsubscribe" => unsubscribe();
    }
    forwards! { host, "tool-invalidator", HostToolInvalidator, InvalidatorEndpoint,
        "invalidate" => invalidate();
    }
    forwards! { host, "bash-output", HostBashOutput, BashOutput,
        "on-data" => on_data(bytes: Vec<u8>);
    }
    Ok(())
}

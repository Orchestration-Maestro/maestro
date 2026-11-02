//! Both registration routes attach the same retained executable provider owner.
use crate::{guest_calls::Handle, local_resources::Registry, provider_calls::Provider};
use maestro_extensions::{ProviderConfig, SharedHandle};
use std::sync::RwLock;

/// Decodes provider metadata and attaches both retained capabilities.
pub(crate) fn config(
    data: &str,
    (stream, oauth): Callbacks,
    registry: &Registry,
) -> Result<SharedHandle<RwLock<ProviderConfig>>, String> {
    let mut config: ProviderConfig = crate::writer_reads::decode(data)?;
    config.callbacks.stream_simple = stream
        .map(|handler| {
            registry.requests().map(|requests| {
                SharedHandle::new(Provider { handler, requests })
                    as maestro_extensions::StreamFunction<maestro_extensions::StreamOptionsInput>
            })
        })
        .transpose()?;
    config.callbacks.oauth = oauth;
    Ok(SharedHandle::new(RwLock::new(config)))
}

/// Streaming and OAuth callback resources carried by the same registration.
pub(crate) type Wire = (
    Option<wasmtime::component::Resource<crate::guest_calls::Callback>>,
    crate::oauth_registration::Wire,
);

/// Independently retained executable capabilities, not part of the JSON metadata.
pub(crate) type Callbacks = (
    Option<Handle>,
    Option<maestro_extensions::OAuthProviderHandle>,
);

/// Retains both callback identities before either registration route crosses to its local owner.
pub(crate) fn retain<T: crate::callback_imports::CallbackView>(
    state: &mut T,
    name: &str,
    (stream, oauth): Wire,
) -> wasmtime::Result<Callbacks> {
    let stream = stream
        .map(|stream| crate::callback_imports::retain(state, &stream))
        .transpose()?;
    let oauth = crate::oauth_registration::retain(state, name, oauth)?;
    Ok((stream, oauth))
}

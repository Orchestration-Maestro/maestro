//! Both registration routes attach the same retained executable provider owner.
use crate::{guest_calls::Handle, local_resources::Registry, provider_calls::Provider};
use maestro_extensions::{ProviderConfig, SharedHandle};
use std::sync::RwLock;

/// Decode ordinary registration data and attach its independently retained callback.
pub(crate) fn config(
    data: &str,
    stream: Option<Handle>,
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
    Ok(SharedHandle::new(RwLock::new(config)))
}

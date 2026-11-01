//! Both registration routes stage all executable capabilities beside one data record.
use super::{Imports, callbacks::Identity, imports::ProviderRegistration};
use crate::{ExtensionResult, ProviderConfig};

/// Retains authored callbacks under their original host identities until native final release.
pub(super) fn register<I: Imports>(
    imports: &I,
    config: ProviderConfig,
    send: impl FnOnce(&str, ProviderRegistration<'_, I::Callback>) -> ExtensionResult<()>,
) -> ExtensionResult<()> {
    let data = crate::types::finite::encode_value(&config)?;
    let stream = config.callbacks.stream_simple.map(|stream| {
        let identity = Identity::new(imports);
        identity.stage(stream);
        identity
    });
    let oauth = config.callbacks.oauth.map(|provider| {
        let identity = Identity::new(imports);
        identity.stage(provider.clone());
        (identity, provider)
    });
    let result = send(
        &data,
        (
            stream.as_ref().map(|identity| &identity.handle),
            oauth.as_ref().map(|(identity, provider)| {
                (
                    &identity.handle,
                    provider.name(),
                    provider.uses_callback_server(),
                )
            }),
        ),
    );
    for identity in stream
        .into_iter()
        .chain(oauth.map(|(identity, _)| identity))
    {
        identity.settle(result.is_ok());
    }
    result
}

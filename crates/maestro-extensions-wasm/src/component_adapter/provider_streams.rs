//! Registered provider callbacks share the original callback identity owner.
use super::{Imports, callbacks::Identity};
use crate::{ExtensionResult, ProviderConfig};

/// Retain executable registration separately from the shared data codec.
pub(super) fn register<I: Imports>(
    imports: &I,
    config: ProviderConfig,
    send: impl FnOnce(&str, Option<&I::Callback>) -> ExtensionResult<()>,
) -> ExtensionResult<()> {
    let data = crate::types::finite::encode_value(&config)?;
    let stream = config.callbacks.stream_simple.map(|stream| {
        let identity = Identity::new(imports);
        identity.stage(stream);
        identity
    });
    let result = send(&data, stream.as_ref().map(|identity| &identity.handle));
    if let Some(stream) = stream {
        stream.settle(result.is_ok());
    }
    result
}

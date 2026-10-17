//! Default cloud-conversation callbacks for installing or restoring bundled invocation.
use crate::BedrockProviderModule;
use crate::SharedHandle;
use crate::builtins::register_builtins::Bedrock;

/// Return the native adapter's raw and simple callbacks, or its unsupported browser callbacks.
#[must_use]
pub fn bedrock_provider_module() -> BedrockProviderModule {
    let owner = SharedHandle::new(Bedrock);
    BedrockProviderModule {
        stream_bedrock: owner.clone(),
        stream_simple_bedrock: owner,
    }
}

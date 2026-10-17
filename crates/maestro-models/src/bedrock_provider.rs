//! Default cloud-conversation callbacks for installing or restoring bundled invocation.
use crate::BedrockProviderModule;
use crate::SharedHandle;
use crate::builtins::{raw_options, register_builtins::raw_callback};
use crate::providers::cloud::amazon_bedrock::{stream_bedrock, stream_simple_bedrock};

/// Return the native adapter's raw and simple callbacks, or its unsupported browser callbacks.
#[must_use]
pub fn bedrock_provider_module() -> BedrockProviderModule {
    BedrockProviderModule {
        stream_bedrock: raw_callback(raw_options::bedrock, stream_bedrock),
        stream_simple_bedrock: SharedHandle::new(|model, context, options| {
            Ok(stream_simple_bedrock(model, context, options))
        }),
    }
}

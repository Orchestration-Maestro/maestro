/// Provider credential discovery from the process environment.
mod env_api_keys;

pub use env_api_keys::{find_env_keys, get_env_api_key};

/// Typed decoding of the JSON extras of each bundled raw protocol.
mod raw_options;
mod register_builtins;

pub(crate) use register_builtins::built_in_providers;
pub use register_builtins::{
    register_built_in_api_providers, reset_api_providers, stream_simple_anthropic,
    stream_simple_azure_openai_responses, stream_simple_mistral, stream_simple_openai_completions,
    stream_simple_openai_responses,
};

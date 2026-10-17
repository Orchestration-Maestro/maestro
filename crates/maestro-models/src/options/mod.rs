//! Shared invocation helpers.
mod simple_options;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use simple_options::thinking_budget;
pub use simple_options::{
    AdjustedMaxTokens, adjust_max_tokens_for_thinking, build_base_options, build_base_options_for,
    clamp_reasoning, reasoning_from_thinking_level,
};
pub(crate) use simple_options::{resolve_api_key, resolve_cache_retention, token_base};

mod overflow;
pub use overflow::is_context_overflow;

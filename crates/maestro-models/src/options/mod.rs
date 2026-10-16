//! Shared invocation helpers.
mod simple_options;
pub use simple_options::{
    AdjustedMaxTokens, adjust_max_tokens_for_thinking, build_base_options, clamp_reasoning,
    reasoning_from_thinking_level,
};
pub(crate) use simple_options::{
    resolve_api_key, resolve_cache_retention, thinking_budget, token_base,
};

mod overflow;
pub use overflow::is_context_overflow;

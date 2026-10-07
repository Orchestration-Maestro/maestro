mod overflow;
mod simple_options;

pub use overflow::{RegExp, get_overflow_patterns, is_context_overflow};
pub use simple_options::{
    AdjustedMaxTokens, adjust_max_tokens_for_thinking, build_base_options, clamp_reasoning,
};

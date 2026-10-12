//! Selection defaults shared by initial choice and scopes.
use maestro_models::ModelThinkingLevel;

/// Thinking level used when neither the caller nor a scope entry supplies one.
pub const DEFAULT_THINKING_LEVEL: ModelThinkingLevel = ModelThinkingLevel::Medium;

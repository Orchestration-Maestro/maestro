//! Built-in model protocol adapters.
pub mod amazon_bedrock;
pub mod anthropic_messages;
pub mod google;
pub mod mistral_conversations;
pub(crate) mod model_compat;
pub mod openai_codex_responses;
pub mod openai_completions;
pub(crate) mod openai_prompt_cache;
pub mod openai_responses;
pub(crate) mod selected_fields;

//! Browser-facing localization, display formatting and SVG glyphs.

pub mod icons;
pub mod utils;
pub use utils::format::{format_cost, format_model_cost, format_token_count, format_usage};
#[cfg(target_arch = "wasm32")]
pub use utils::i18n::BrowserLanguageHost;
pub use utils::i18n::{
    I18n, I18nDiagnostic, I18nError, I18nMessages, LanguageCode, LanguageHost, TranslationFunction,
    TranslationValue, Translations,
};

pub use maestro_app::browser::components::messages::{
    ArtifactMessage, UserMessageWithAttachments, convert_attachments, default_convert_to_llm,
    is_artifact_message, is_user_message_with_attachments,
};
pub use maestro_app::browser::utils::attachment_utils::{Attachment, load_attachment};

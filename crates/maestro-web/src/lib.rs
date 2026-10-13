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

//! Browser display utilities.
pub mod formatting;
pub mod localization;

/// Image, text and presentation attachment loading.
pub mod attachments;

/// Provider-specific CORS proxy policy.
pub mod proxy;

/// Local-server model discovery.
pub mod model_discovery;

#[cfg(target_arch = "wasm32")]
pub(crate) mod download;

pub(crate) mod base64;

#[cfg(target_arch = "wasm32")]
pub(crate) mod clipboard;

/// Browser-local token kept in local storage.
#[cfg(target_arch = "wasm32")]
pub mod authorization;

/// Parse the signed leading ASCII decimal digits after text whitespace.
pub(crate) fn parse_decimal_prefix(text: &str) -> Result<f64, std::num::ParseFloatError> {
    let text = text.trim_start_matches(maestro_text::is_whitespace_scalar);
    let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
    let length = digits.bytes().take_while(u8::is_ascii_digit).count();
    text[..text.len() - digits.len() + length].parse()
}

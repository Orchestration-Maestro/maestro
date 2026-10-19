//! Browser attachment and provider utilities.

pub use maestro_models::arguments::json_parse::parse_json;
/// Image, text and presentation attachment loading.
pub mod attachment_utils;

/// Provider-specific CORS proxy policy.
pub mod proxy_utils;

/// Local-server model discovery.
pub mod model_discovery;

/// Parse the signed leading ASCII decimal digits after text whitespace.
pub(crate) fn parse_decimal_prefix(text: &str) -> Result<f64, std::num::ParseFloatError> {
    let text = text.trim_start_matches(maestro_text::is_whitespace_scalar);
    let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
    let length = digits.bytes().take_while(u8::is_ascii_digit).count();
    text[..text.len() - digits.len() + length].parse()
}

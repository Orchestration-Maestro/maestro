//! Account selection from request credentials.

use crate::DiagnosticErrorInfo;
use crate::providers::json_text::{member, raw_json};
use base64::{
    Engine as _, alphabet,
    engine::{GeneralPurpose, GeneralPurposeConfig},
};

/// Read the account claim without signature verification.
pub(super) fn extract_account_id(token: &str) -> Result<String, DiagnosticErrorInfo> {
    let failure = || DiagnosticErrorInfo {
        name: Some("Error".to_owned()),
        message: "Failed to extract accountId from token".to_owned(),
        stack: None,
        code: None,
    };
    let parts: Vec<_> = token.split('.').collect();
    let [_, payload, _] = parts.as_slice() else {
        return Err(failure());
    };
    let config = GeneralPurposeConfig::new()
        .with_decode_padding_mode(base64::engine::DecodePaddingMode::Indifferent)
        .with_decode_allow_trailing_bits(true);
    let bytes = GeneralPurpose::new(&alphabet::STANDARD, config)
        .decode(payload)
        .map_err(|_| failure())?;
    let text: String = bytes.into_iter().map(char::from).collect();
    let raw = raw_json(&text).map_err(|_| failure())?;
    let account = member(raw, "https://api.openai.com/auth")
        .and_then(|claims| member(claims, "chatgpt_account_id"))
        .and_then(|account| serde_json::from_str::<String>(account.get()).ok())
        .filter(|account| !account.is_empty());
    account.ok_or_else(failure)
}

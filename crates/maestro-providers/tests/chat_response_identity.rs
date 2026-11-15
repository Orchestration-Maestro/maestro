//! Controlled public chat qualification.

#[cfg(not(target_arch = "wasm32"))]
use crate::chat;
#[cfg(not(target_arch = "wasm32"))]
use crate::chat_qualification as qualification;

#[cfg(not(target_arch = "wasm32"))]
use chat::TestResult;

#[cfg(not(target_arch = "wasm32"))]
/// First distinct nonempty reported model and first nonempty response identifier stay retained.
#[test]
fn maestro_chat_retains_routed_model_identity() -> TestResult {
    qualification::assert_rows("maestro_chat_retains_routed_model_identity")
}

#[cfg(not(target_arch = "wasm32"))]
/// Provider finish reasons retain partial text and the terminal outcome.
#[test]
fn maestro_chat_preserves_provider_finish_outcomes() -> TestResult {
    qualification::assert_rows("maestro_chat_preserves_provider_finish_outcomes")
}

#[cfg(not(target_arch = "wasm32"))]
/// A null wire chunk does not hide the following valid response.
#[test]
fn maestro_chat_ignores_null_wire_chunks() -> TestResult {
    qualification::assert_rows("maestro_chat_ignores_null_wire_chunks")
}

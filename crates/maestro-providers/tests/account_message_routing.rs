//! Controlled public message qualification.

#[cfg(not(target_arch = "wasm32"))]
use crate::chat;
#[cfg(not(target_arch = "wasm32"))]
use crate::message_qualification as qualification;

#[cfg(not(target_arch = "wasm32"))]
use chat::TestResult;

#[cfg(not(target_arch = "wasm32"))]
/// Catalog account routes retain bearer, initiator and image-specific headers.
#[test]
fn account_routes_message_authentication() -> TestResult {
    qualification::assert_rows("account_routes_message_authentication")
}

#[cfg(not(target_arch = "wasm32"))]
/// Interleaved beta options depend on model family independently of thinking enablement.
#[test]
fn account_selects_interleaved_beta_independently() -> TestResult {
    qualification::assert_rows("account_selects_interleaved_beta_independently")
}

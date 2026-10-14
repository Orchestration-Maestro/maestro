//! Controlled public message qualification.

use crate::chat;
use crate::message_qualification as qualification;

use chat::TestResult;

/// Catalog account routes retain bearer, initiator and image-specific headers.
#[test]
fn maestro_account_routes_message_authentication() -> TestResult {
    qualification::assert_rows("maestro_account_routes_message_authentication")
}

/// Interleaved beta options depend on model family independently of thinking enablement.
#[test]
fn maestro_account_selects_interleaved_beta_independently() -> TestResult {
    qualification::assert_rows("maestro_account_selects_interleaved_beta_independently")
}

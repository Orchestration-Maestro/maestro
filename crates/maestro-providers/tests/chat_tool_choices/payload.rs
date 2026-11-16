//! Controlled public dispatch observations.

#[cfg(not(target_arch = "wasm32"))]
use crate::chat_tool_choices::{chat::TestResult, qualification};

#[cfg(not(target_arch = "wasm32"))]
/// Raw and simple public routes forward each admitted tool choice.
#[test]
fn chat_public_choices_reach_wire() -> TestResult {
    qualification::assert_rows("chat_public_choices_reach_wire")
}

#[cfg(not(target_arch = "wasm32"))]
/// Disabled strict-mode support omits the strict member.
#[test]
fn chat_omits_disabled_strictness() -> TestResult {
    qualification::assert_rows("chat_omits_disabled_strictness")
}

#[cfg(not(target_arch = "wasm32"))]
/// Catalog reasoning maps apply at each requested level.
#[test]
fn chat_uses_catalog_reasoning_maps() -> TestResult {
    qualification::assert_rows("chat_uses_catalog_reasoning_maps")
}

#[cfg(not(target_arch = "wasm32"))]
/// Tool streaming follows catalog compatibility and explicit overrides.
#[test]
fn chat_tool_stream_follows_catalog_and_overrides() -> TestResult {
    qualification::assert_rows("chat_tool_stream_follows_catalog_and_overrides")
}

#[cfg(not(target_arch = "wasm32"))]
/// Routed reasoning uses the nested effort object.
#[test]
fn router_uses_nested_reasoning() -> TestResult {
    qualification::assert_rows("router_uses_nested_reasoning")
}

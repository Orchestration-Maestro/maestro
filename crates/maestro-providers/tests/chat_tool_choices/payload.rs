//! Controlled public dispatch observations.

#[cfg(not(target_arch = "wasm32"))]
use crate::chat_tool_choices::{chat::TestResult, qualification};

#[cfg(not(target_arch = "wasm32"))]
/// Raw and simple public routes forward each admitted tool choice.
#[test]
fn maestro_chat_public_choices_reach_wire() -> TestResult {
    qualification::assert_rows("maestro_chat_public_choices_reach_wire")
}

#[cfg(not(target_arch = "wasm32"))]
/// Disabled strict-mode support omits the strict member.
#[test]
fn maestro_chat_omits_disabled_strictness() -> TestResult {
    qualification::assert_rows("maestro_chat_omits_disabled_strictness")
}

#[cfg(not(target_arch = "wasm32"))]
/// Catalog reasoning maps apply at each requested level.
#[test]
fn maestro_chat_uses_catalog_reasoning_maps() -> TestResult {
    qualification::assert_rows("maestro_chat_uses_catalog_reasoning_maps")
}

#[cfg(not(target_arch = "wasm32"))]
/// Tool streaming follows catalog compatibility and explicit overrides.
#[test]
fn maestro_chat_tool_stream_follows_catalog_and_overrides() -> TestResult {
    qualification::assert_rows("maestro_chat_tool_stream_follows_catalog_and_overrides")
}

#[cfg(not(target_arch = "wasm32"))]
/// Routed reasoning uses the nested effort object.
#[test]
fn maestro_router_uses_nested_reasoning() -> TestResult {
    qualification::assert_rows("maestro_router_uses_nested_reasoning")
}

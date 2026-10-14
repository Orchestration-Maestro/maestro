//! Controlled public dispatch observations.

use crate::chat_tool_choices::{chat::TestResult, qualification};

/// Raw and simple public routes forward each admitted tool choice.
#[test]
fn maestro_chat_public_choices_reach_wire() -> TestResult {
    qualification::assert_rows("maestro_chat_public_choices_reach_wire")
}

/// Disabled strict-mode support omits the strict member.
#[test]
fn maestro_chat_omits_disabled_strictness() -> TestResult {
    qualification::assert_rows("maestro_chat_omits_disabled_strictness")
}

/// Catalog reasoning maps apply at each requested level.
#[test]
fn maestro_chat_uses_catalog_reasoning_maps() -> TestResult {
    qualification::assert_rows("maestro_chat_uses_catalog_reasoning_maps")
}

/// Tool streaming follows catalog compatibility and explicit overrides.
#[test]
fn maestro_chat_tool_stream_follows_catalog_and_overrides() -> TestResult {
    qualification::assert_rows("maestro_chat_tool_stream_follows_catalog_and_overrides")
}

/// Routed reasoning uses the nested effort object.
#[test]
fn maestro_router_uses_nested_reasoning() -> TestResult {
    qualification::assert_rows("maestro_router_uses_nested_reasoning")
}

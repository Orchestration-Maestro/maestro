//! Recorded transcript frames through public component operations.
#![cfg(test)]
#[path = "message_support/mod.rs"]
mod message_support;

#[test]
fn user_markers_preserve_padded_height() {
    message_support::group("user_markers_preserve_padded_height");
}

#[test]
fn user_empty_and_wrapped_markdown() {
    message_support::group("user_empty_and_wrapped_markdown");
}

#[test]
fn assistant_zone_markers_follow_tool_presence() {
    message_support::group("assistant_zone_markers_follow_tool_presence");
}

#[test]
fn assistant_empty_blocks_render_nothing() {
    message_support::group("assistant_empty_blocks_render_nothing");
}

#[test]
fn assistant_order_keeps_only_required_thinking_gaps() {
    message_support::group("assistant_order_keeps_only_required_thinking_gaps");
}

#[test]
fn assistant_trimming_uses_shared_whitespace() {
    message_support::group("assistant_trimming_uses_shared_whitespace");
}

#[test]
fn assistant_hidden_labels_follow_constructor_and_setters() {
    message_support::group("assistant_hidden_labels_follow_constructor_and_setters");
}

#[test]
fn hidden_thinking_respects_decoration_enablement() {
    message_support::group("hidden_thinking_respects_decoration_enablement");
}

#[test]
fn assistant_notices_keep_fallbacks_and_tool_suppression() {
    message_support::group("assistant_notices_keep_fallbacks_and_tool_suppression");
}

#[test]
fn assistant_updates_replace_content_and_marker_policy() {
    message_support::group("assistant_updates_replace_content_and_marker_policy");
}

#[test]
fn assistant_invalidation_reuses_retained_message() {
    message_support::group("assistant_invalidation_reuses_retained_message");
}

#[test]
fn assistant_releases_replaced_message_handles() {
    message_support::releases_handles();
}

#[test]
fn assistant_text_and_thinking_drop_terminal_controls() {
    message_support::drops_terminal_controls();
}

#[test]
fn branch_summary_expands_and_collapses() {
    message_support::group("branch_summary_expands_and_collapses");
}

#[test]
fn compaction_summary_expands_and_collapses() {
    message_support::group("compaction_summary_expands_and_collapses");
}

#[test]
fn custom_markdown_theme_reaches_each_message_kind() {
    message_support::group("custom_markdown_theme_reaches_each_message_kind");
}

#[test]
fn message_styles_refresh_with_live_theme() {
    message_support::group("message_styles_refresh_with_live_theme");
}

#[test]
fn message_children_keep_container_alias_semantics() {
    message_support::group("message_children_keep_container_alias_semantics");
}

#[test]
fn message_panels_use_shipped_brand_roles() {
    message_support::group("message_panels_use_shipped_brand_roles");
}

#[test]
fn summary_hints_refresh_from_live_registry() {
    message_support::group("summary_hints_refresh_from_live_registry");
}

#[test]
fn summary_invalidation_rebuilds_edited_children() {
    message_support::group("summary_invalidation_rebuilds_edited_children");
}

#[test]
fn summary_empty_text_keeps_its_header() {
    message_support::group("summary_empty_text_keeps_its_header");
}

#[test]
fn compaction_count_uses_locale_formatter() {
    message_support::group("compaction_count_uses_locale_formatter");
}

/// Each fixture group has precisely one public replay test.
const FIXTURE_GROUPS: &[&str] = &[
    "user_markers_preserve_padded_height",
    "user_empty_and_wrapped_markdown",
    "assistant_zone_markers_follow_tool_presence",
    "assistant_empty_blocks_render_nothing",
    "assistant_order_keeps_only_required_thinking_gaps",
    "assistant_trimming_uses_shared_whitespace",
    "assistant_hidden_labels_follow_constructor_and_setters",
    "hidden_thinking_respects_decoration_enablement",
    "assistant_notices_keep_fallbacks_and_tool_suppression",
    "assistant_updates_replace_content_and_marker_policy",
    "assistant_invalidation_reuses_retained_message",
    "branch_summary_expands_and_collapses",
    "compaction_summary_expands_and_collapses",
    "custom_markdown_theme_reaches_each_message_kind",
    "message_styles_refresh_with_live_theme",
    "message_children_keep_container_alias_semantics",
    "message_panels_use_shipped_brand_roles",
    "summary_hints_refresh_from_live_registry",
    "summary_invalidation_rebuilds_edited_children",
    "summary_empty_text_keeps_its_header",
    "compaction_count_uses_locale_formatter",
];

#[test]
fn branch_summary_drops_terminal_controls() {
    message_support::summary_controls("branch");
}

#[test]
fn compaction_summary_drops_terminal_controls() {
    message_support::summary_controls("compaction");
}

#[test]
fn user_message_keeps_authored_terminal_styles() {
    message_support::user_styles();
}

#[test]
fn adjacent_thinking_blocks_render_as_one_section() {
    message_support::adjacent_thinking();
}

#[test]
fn user_message_is_one_padded_markdown_layer() {
    message_support::user_markdown_layer();
}

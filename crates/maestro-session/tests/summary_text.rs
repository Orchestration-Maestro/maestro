//! Shared summary-input formatting through the public interface.
#![cfg(test)]
/// Recorded operation support.
mod support {
    /// Compaction corpus consumer.
    pub mod compaction;
    /// Complete numeric comparison.
    pub mod value;
}
#[test]
fn file_sections_preserve_tags_order_and_separators() {
    support::compaction::group("file_format");
}

#[test]
fn conversation_clips_only_tool_results() {
    support::compaction::group("serialization_limits");
}

#[test]
fn conversation_groups_categories_and_keeps_argument_order() {
    support::compaction::group("serialization_format");
}

#[test]
fn tool_result_cutoff_preserves_unicode_scalars() {
    support::compaction::group("scalar_clipping");
}

#[test]
fn conversation_preserves_text_and_nested_json_boundaries() {
    support::compaction::group("serialization_boundaries");
}

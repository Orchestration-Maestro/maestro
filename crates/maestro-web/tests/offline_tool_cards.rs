//! Offline tool cards through the exported formatter.
#![cfg(test)]
#[cfg(test)]
mod tests {
    use crate::offline_tool_card_support::replay;

    /// First matching result controls the complete shell and output.
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
    pub fn tool_cards_keep_status_and_first_result() {
        replay("tool_cards_keep_status_and_first_result");
    }
    /// Shell display keeps the exact outer whitespace policy and preview boundary.
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
    pub fn tool_shell_cards_trim_only_outer_ecma_whitespace() {
        replay("tool_shell_cards_trim_only_outer_ecma_whitespace");
    }
    /// File headers preserve precedence, lexical shortening and numeric ranges.
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
    pub fn tool_file_cards_keep_path_precedence_and_line_ranges() {
        replay("tool_file_cards_keep_path_precedence_and_line_ranges");
    }
    /// Replay every complete input owned by this card behavior.
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
    pub fn tool_reads_keep_blank_lines_language_and_images() {
        replay("tool_reads_keep_blank_lines_language_and_images");
    }
    /// Replay every complete input owned by this card behavior.
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
    pub fn tool_image_metadata_cannot_create_attributes() {
        replay("tool_image_metadata_cannot_create_attributes");
    }
    /// Replay every complete input owned by this card behavior.
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
    pub fn tool_image_metadata_uses_persisted_display_values() {
        replay("tool_image_metadata_uses_persisted_display_values");
    }
    /// Replay every complete input owned by this card behavior.
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
    pub fn tool_writes_keep_content_preview_and_trimmed_result() {
        replay("tool_writes_keep_content_preview_and_trimmed_result");
    }
    /// Replay every complete input owned by this card behavior.
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
    pub fn tool_edits_keep_diff_roles_and_fallback() {
        replay("tool_edits_keep_diff_roles_and_fallback");
    }
    /// Replay every complete input owned by this card behavior.
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
    pub fn tool_directory_cards_keep_limit_display_and_twenty_line_preview() {
        replay("tool_directory_cards_keep_limit_display_and_twenty_line_preview");
    }
    /// Replay every complete input owned by this card behavior.
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
    pub fn tool_custom_cards_keep_all_renderer_modes() {
        replay("tool_custom_cards_keep_all_renderer_modes");
    }
    /// Replay every complete input owned by this card behavior.
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
    pub fn tool_structured_fallback_keeps_canonical_json_and_literal_text() {
        replay("tool_structured_fallback_keeps_canonical_json_and_literal_text");
        let doc = crate::offline_tool_card_support::document(&serde_json::json!({"entries":[]}));
        let call = serde_json::json!({"id":"zero", "name":"custom", "arguments":{"zero":-0.0}});
        assert!(
            call["arguments"]["zero"]
                .as_f64()
                .unwrap()
                .is_sign_negative()
        );
        let html = maestro_web::export_html::template::render_tool_call(
            &doc,
            &call,
            &crate::highlighter_support::NoHighlight,
        )
        .unwrap();
        assert!(html.contains("<pre>{\n  &quot;zero&quot;: 0\n}</pre>"));
    }
    /// Replay every complete input owned by this card behavior.
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
    pub fn tool_cards_report_reached_invalid_data_without_partial_output() {
        replay("tool_cards_report_reached_invalid_data_without_partial_output");
    }
    /// Replay every complete input owned by this card behavior.
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
    pub fn tool_result_fields_keep_all_persisted_value_kinds() {
        replay("tool_result_fields_keep_all_persisted_value_kinds");
    }
    /// Replay every complete input owned by this card behavior.
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
    pub fn tool_ranges_keep_invalid_operands_literal() {
        replay("tool_ranges_keep_invalid_operands_literal");
    }
    /// Replay every complete input owned by this card behavior.
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
    pub fn tool_cards_keep_record_errors_local_to_each_card() {
        replay("tool_cards_keep_record_errors_local_to_each_card");
    }
}

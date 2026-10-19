//! Tool cards and disclosures through the real browser host.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::tool_cards_support;
#[cfg(test)]
mod tests {
    use super::*;
    use tool_cards_support::surface;
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    pub fn bash_cards_select_all_parameter_and_result_branches() {
        tool_cards_support::rows("bash_cards_select_all_parameter_and_result_branches");
    }
    #[wasm_bindgen]
    pub fn calculate_cards_select_all_parameter_and_result_branches() {
        tool_cards_support::rows("calculate_cards_select_all_parameter_and_result_branches");
    }
    #[wasm_bindgen]
    pub fn time_cards_select_all_parameter_and_result_branches() {
        tool_cards_support::rows("time_cards_select_all_parameter_and_result_branches");
    }
    #[wasm_bindgen]
    pub fn tool_cards_keep_ordered_literal_text_and_escaping() {
        tool_cards_support::rows("tool_cards_keep_ordered_literal_text_and_escaping");
    }
    #[wasm_bindgen]
    pub fn tool_cards_share_truthy_field_projection_for_all_result_states() {
        use maestro_web::tools::types::ToolRenderState;
        let mut surface = surface();
        let mut keys = std::collections::HashSet::new();
        for row in tool_cards_support::fixture().domains {
            let input = serde_json::to_string(&(
                &row.tool,
                &row.input.params,
                &row.input.result,
                row.input.is_streaming,
            ))
            .unwrap();
            assert!(keys.insert(input), "duplicate selected-field input");
            assert_eq!(
                row.input
                    .result
                    .as_ref()
                    .map_or("pending", |result| if result.is_error {
                        "error"
                    } else {
                        "success"
                    }),
                row.phase
            );
            let field = match row.tool.as_str() {
                "bash" => "command",
                "calculate" => "expression",
                "time" => "timezone",
                _ => panic!("unknown tool"),
            };
            let params = row.input.params.as_ref().unwrap();
            assert_eq!(params.contains_key(field), row.kind != "missing");
            if row.kind == "negative-zero" {
                assert!(params[field].as_f64().unwrap().is_sign_negative());
            }
            let recipe = tool_cards_support::prepare(&row.tool, &row.input);
            let mut state = ToolRenderState::default();
            tool_cards_support::observe(&mut surface, &recipe, &mut state, &row.expected);
        }
        assert_eq!(keys.len(), 108);
    }
    #[wasm_bindgen]
    pub fn tool_cards_localize_captions_without_localizing_literals() {
        use maestro_web::tools::types::ToolRenderState;
        let mut surface = surface();
        for mut row in tool_cards_support::fixture().localized {
            let language = row.input.language.take().unwrap();
            assert_eq!(language, "de");
            surface.i18n.set_language(&language).unwrap();
            let recipe = tool_cards_support::prepare(&row.tool, &row.input);
            let mut state = ToolRenderState::default();
            tool_cards_support::observe(&mut surface, &recipe, &mut state, &row.expected);
        }
    }
    #[wasm_bindgen]
    pub fn expandable_sections_keep_owned_children_through_toggles_and_reconnects() {
        tool_cards_support::disclosure::lifecycle();
    }
    #[wasm_bindgen]
    pub async fn expandable_sections_admit_visible_pointer_and_keyboard_activation() {
        tool_cards_support::disclosure::activation().await;
    }
    #[wasm_bindgen]
    pub fn tool_cards_reach_the_composed_message_consumer() {
        tool_cards_support::composed::composed();
    }
    #[wasm_bindgen]
    pub fn tool_cards_release_only_disappearing_native_bodies() {
        tool_cards_support::lifetimes::lifetimes();
    }
    #[wasm_bindgen]
    pub fn expandable_sections_center_wrapped_summary_and_hover_lower_line() {
        tool_cards_support::styles::wrapped_summary();
    }
    #[wasm_bindgen]
    pub fn tool_cards_and_sections_follow_selected_brand_and_final_clip() {
        tool_cards_support::styles::styles();
    }
}

//! Document cards observed through the controlled native browser host.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::document_card_support::browser as cards;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn repl_cards_cover_branch_and_status_precedence() {
    cards::rows(|r| r.test.as_deref() == Some("repl_cards_cover_branch_and_status_precedence"));
}

#[wasm_bindgen]
pub fn extract_cards_cover_branch_and_status_precedence() {
    cards::rows(|r| r.test.as_deref() == Some("extract_cards_cover_branch_and_status_precedence"));
}
#[wasm_bindgen]
pub fn document_outputs_keep_order_empty_lines_and_literal_code() {
    cards::rows(|r| {
        r.test.as_deref() == Some("card_outputs_preserve_order_literal_text_and_result_branch")
    });
}
#[wasm_bindgen]
pub fn document_cards_localize_only_localized_captions() {
    cards::rows(|r| r.group == "localization");
}
#[wasm_bindgen]
pub fn extraction_metadata_preserves_names_case_units_and_rounding() {
    cards::rows(|r| matches!(r.group.as_str(), "metadata" | "malformed"));
}
#[wasm_bindgen]
pub fn document_field_policy_keeps_missing_null_falsy_and_composites() {
    cards::rows(|r| r.group == "domains");
}
#[wasm_bindgen]
pub fn document_disclosures_keep_mounted_children_and_transitions() {
    crate::document_card_support::lifecycle::transitions();
}

#[wasm_bindgen]
pub fn document_card_branches_reset_only_replaced_subtrees() {
    crate::document_card_support::lifecycle::branches();
    crate::document_card_support::lifecycle::renderer_switch();
}
#[wasm_bindgen]
pub fn document_cards_keep_independent_rows_and_release_surfaces() {
    crate::document_card_support::lifecycle::independent();
}
#[wasm_bindgen]
pub fn returned_file_tiles_open_independent_overlays_while_collapsed() {
    crate::document_card_support::overlays::returned_files();
}
#[wasm_bindgen]
pub fn assistant_and_rich_content_forward_opened_overlays_before_errors() {
    crate::document_card_support::overlays::assistant_and_children();
}
#[wasm_bindgen]
pub fn user_and_tool_messages_share_one_overlay_receiver() {
    crate::document_card_support::overlays::user_and_tool();
}
#[wasm_bindgen]
pub async fn document_controls_keep_keyboard_pointer_clip_and_modal_routes() {
    crate::document_card_support::lifecycle::controls().await;
}
#[wasm_bindgen]
pub fn document_cards_use_selected_brand_roles_and_keep_operation_errors() {
    crate::document_card_support::styles::run();
}

//! Document cards observed through the controlled native browser host.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::document_card_support::{browser as cards, downloads};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn repl_cards_cover_branch_and_status_precedence(import: &js_sys::Function) {
    cards::rows(import, |r| {
        r.test.as_deref() == Some("repl_cards_cover_branch_and_status_precedence")
    });
}

#[wasm_bindgen]
pub fn extract_cards_cover_branch_and_status_precedence(import: &js_sys::Function) {
    cards::rows(import, |r| {
        r.test.as_deref() == Some("extract_cards_cover_branch_and_status_precedence")
    });
}
#[wasm_bindgen]
pub fn document_outputs_keep_order_empty_lines_and_literal_code(import: &js_sys::Function) {
    cards::rows(import, |r| {
        r.test.as_deref() == Some("card_outputs_preserve_order_literal_text_and_result_branch")
    });
}
#[wasm_bindgen]
pub fn document_cards_localize_only_localized_captions(import: &js_sys::Function) {
    cards::rows(import, |r| r.group == "localization");
}
#[wasm_bindgen]
pub fn extraction_metadata_preserves_names_case_units_and_rounding(import: &js_sys::Function) {
    cards::rows(import, |r| {
        matches!(r.group.as_str(), "metadata" | "malformed")
    });
}
#[wasm_bindgen]
pub fn document_field_policy_keeps_missing_null_falsy_and_composites(import: &js_sys::Function) {
    cards::rows(import, |r| r.group == "domains");
}
#[wasm_bindgen]
pub async fn document_disclosures_keep_mounted_children_and_transitions(import: &js_sys::Function) {
    crate::document_card_support::lifecycle::transitions(import).await;
}

#[wasm_bindgen]
pub fn document_card_branches_reset_only_replaced_subtrees(import: &js_sys::Function) {
    crate::document_card_support::lifecycle::branches(import);
    crate::document_card_support::lifecycle::renderer_switch(import);
}
#[wasm_bindgen]
pub async fn document_cards_keep_independent_rows_and_release_surfaces(import: &js_sys::Function) {
    crate::document_card_support::lifecycle::independent(import).await;
}
#[wasm_bindgen]
pub fn returned_file_tiles_open_independent_overlays_while_collapsed(import: &js_sys::Function) {
    crate::document_card_support::overlays::returned_files(import);
}
#[wasm_bindgen]
pub fn assistant_and_rich_content_forward_opened_overlays_before_errors(import: &js_sys::Function) {
    crate::document_card_support::overlays::assistant_and_children(import);
}
#[wasm_bindgen]
pub fn user_and_tool_messages_share_one_overlay_receiver(import: &js_sys::Function) {
    crate::document_card_support::overlays::user_and_tool(import);
}
#[wasm_bindgen]
pub async fn document_controls_keep_keyboard_pointer_clip_and_modal_routes(
    import: &js_sys::Function,
) {
    crate::document_card_support::lifecycle::controls(import).await;
}
#[wasm_bindgen]
pub fn document_cards_use_selected_brand_roles_and_keep_operation_errors(
    import: &js_sys::Function,
) {
    crate::document_card_support::styles::run(import);
}

/// Returned-file MIME defaults, duplicate names, previews and byte order use actual native controls.
#[wasm_bindgen]
pub async fn repl_downloads_preserve_mime_defaults_order_and_bytes(import: &js_sys::Function) {
    downloads::records(import, downloads::all_files()).await;
}
/// Text previews decode UTF-8 while downloads keep their original bytes.
#[wasm_bindgen]
pub async fn repl_text_downloads_decode_utf8_without_losing_bytes(import: &js_sys::Function) {
    downloads::records(
        import,
        vec![crate::document_card_support::file_subset(&[1, 3, 12, 13])],
    )
    .await;
}
/// Invalid encoded files still open and reject invalid byte downloads.
#[wasm_bindgen]
pub async fn malformed_repl_downloads_keep_original_content(import: &js_sys::Function) {
    downloads::malformed(import).await;
}
/// A later row failure leaves an independently opened original downloadable.
#[wasm_bindgen]
pub async fn all_render_outputs_keep_opened_overlays_when_drawing_fails(import: &js_sys::Function) {
    crate::document_card_support::forwarding::registered(import).await;
}
/// Rebinding a returned file leaves already opened original bytes unchanged.
#[wasm_bindgen]
pub async fn returned_file_updates_preserve_opened_original_downloads(import: &js_sys::Function) {
    downloads::changed_file_keeps_opened_original(import).await;
}

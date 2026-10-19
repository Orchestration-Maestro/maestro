//! Artifact cards observed through their native browser surfaces.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::artifact_card_support::browser as cards;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub async fn artifact_cards_cover_command_status_and_parameter_branches() {
    cards::rows(
        "artifact_cards_cover_command_status_and_parameter_branches",
        54,
    )
    .await;
}
#[wasm_bindgen]
pub async fn artifact_cards_keep_literal_output_and_html_console_rules() {
    cards::rows(
        "artifact_cards_keep_literal_output_and_html_console_rules",
        288,
    )
    .await;
}
#[wasm_bindgen]
pub async fn artifact_cards_place_pills_only_on_the_selected_header_branches() {
    cards::rows(
        "artifact_cards_place_pills_only_on_the_selected_header_branches",
        21,
    )
    .await;
}
#[wasm_bindgen]
pub async fn artifact_update_distinguishes_absent_and_empty_replacements() {
    cards::rows(
        "artifact_update_distinguishes_absent_and_empty_replacements",
        27,
    )
    .await;
}
#[wasm_bindgen]
pub fn artifact_languages_keep_literal_suffix_rules() {
    cards::languages();
}
#[wasm_bindgen]
pub async fn artifact_fields_are_read_only_by_the_reached_branch() {
    cards::fields().await;
}

#[wasm_bindgen]
pub fn artifact_diffs_keep_ordered_runs_prefixes_and_line_endings() {
    crate::artifact_card_support::browser::diffs();
}

#[wasm_bindgen]
pub fn artifact_recipes_own_selected_inputs_and_panel_handles() {
    crate::artifact_card_support::ownership::run();
}

#[wasm_bindgen]
pub fn artifact_pills_open_existing_files_without_toggling_details() {
    crate::artifact_card_support::pills::actions();
}

#[wasm_bindgen]
pub fn artifact_pills_keep_literal_filenames_and_native_selection() {
    crate::artifact_card_support::pills::literal();
}

#[wasm_bindgen]
pub fn artifact_diff_direct_calls_keep_title_counts_and_class() {
    crate::artifact_card_support::diff::run();
}

#[wasm_bindgen]
pub async fn artifact_cards_localize_captions_without_editing_file_data() {
    cards::localized().await;
}

#[wasm_bindgen]
pub fn artifact_disclosures_keep_children_through_reversals_and_updates() {
    crate::artifact_card_support::lifecycle::templates();
    crate::artifact_card_support::lifecycle::reversals();
    crate::artifact_card_support::lifecycle::shared_headers();
}

#[wasm_bindgen]
pub fn artifact_views_release_only_removed_children() {
    crate::artifact_card_support::lifecycle::release();
}

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

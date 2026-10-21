//! Artifact cards observed through their native browser surfaces.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::artifact_card_support::{browser as cards, browser_records as records};
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
    use crate::support::strict_record;
    let valid: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/artifact_unused_diff_browser.json")).unwrap();
    strict_record::<records::UnusedDiff>(&valid[0], &["input", "expected"]);
    strict_record::<records::Branch>(&valid[0]["expected"], &["branch", "card"]);
    strict_record::<records::Branch>(&valid[2]["expected"], &["branch", "kind"]);
    strict_record::<records::Expected>(
        &valid[0]["expected"]["card"],
        &[
            "isCustom",
            "status",
            "caption",
            "collapsible",
            "code",
            "console",
            "errors",
            "diff",
        ],
    );
    strict_record::<records::CodeBlock>(
        &valid[0]["expected"]["card"]["code"][0],
        &["code", "language", "decoded"],
    );
    strict_record::<records::Console>(
        &valid[0]["expected"]["card"]["console"][0],
        &["content", "variant"],
    );
    let mut wrong = valid[0]["expected"].clone();
    wrong["branch"] = "unknown".into();
    assert!(serde_json::from_value::<records::Branch>(wrong).is_err());

    let valid: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/artifact_code_domains_browser.json")).unwrap();
    crate::support::strict_record::<records::CodeDomain>(&valid[0], &["input", "expected"]);
    for branch in ["omitted", "text"] {
        let expected = valid
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["expected"]["branch"] == branch)
            .unwrap();
        let fields: &[&str] = if branch == "text" {
            &["branch", "text"]
        } else {
            &["branch"]
        };
        crate::support::strict_record::<records::CodeSelection>(&expected["expected"], fields);
    }
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
    use crate::support::strict_record;
    let valid: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/artifact_diff_views.json")).unwrap();
    strict_record::<records::DiffCase>(&valid[3], &["input", "expected"]);
    strict_record::<records::DiffInput>(&valid[3]["input"], &["oldText", "newText", "className"]);
    strict_record::<records::DiffExpected>(&valid[3]["expected"], &["rows", "classPresent"]);
    strict_record::<records::DiffRow>(&valid[3]["expected"]["rows"][0], &["text", "kind"]);

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
    crate::artifact_card_support::composed::rich_header();
    crate::artifact_card_support::composed::caption_without_pill();
}

#[wasm_bindgen]
pub async fn artifact_views_release_only_removed_children() {
    crate::artifact_card_support::lifecycle::release();
    crate::artifact_card_support::lifecycle::copy_feedback().await;
}

#[wasm_bindgen]
pub fn artifact_cards_reach_registered_message_rows_with_native_details() {
    crate::artifact_card_support::composed::run();
}

#[wasm_bindgen]
pub async fn artifact_cards_and_diffs_follow_the_selected_brand_and_final_clip() {
    crate::artifact_card_support::styles::run().await;
}

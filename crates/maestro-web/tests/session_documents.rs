//! Owned document preparation through the public offline reader.
#![cfg(test)]
mod session_document_support;
use session_document_support::{cases, replay};

#[test]
fn document_keeps_utf8_and_raw_export_fields() {
    for row in cases(&[
        "bom",
        "invalid-utf8-inside-json-string",
        "opaque-domain-array",
        "opaque-domain-boolean",
        "opaque-domain-missing",
        "opaque-domain-null",
        "opaque-domain-number",
        "opaque-domain-object",
        "opaque-domain-string",
        "unicode-unknown-fields",
    ]) {
        replay(&row);
    }
}

#[test]
fn document_keeps_canonical_json_and_signed_zero() {
    for row in cases(&["canonical-keys-signed-zero", "duplicate-json-members"]) {
        replay(&row);
    }
}

#[test]
fn document_rejects_bad_payloads_at_loading() {
    for row in cases(&[
        "base64-ascii-whitespace",
        "base64-impossible-length",
        "base64-nonzero-unused-bits",
        "base64-partial-padding",
        "base64-unpadded",
        "base64-url-alphabet",
        "base64-vertical-tab",
        "empty-payload",
        "entries-object",
        "invalid-base64",
        "invalid-json",
        "missing-entries",
        "null-document",
        "null-entries",
        "null-entry",
        "root-array",
        "root-string",
        "second-bom-remains-json-input",
    ]) {
        replay(&row);
    }
}

#[test]
fn document_links_select_injected_params_before_query() {
    for row in cases(&[
        "default-leaf-domain-array",
        "default-leaf-domain-boolean",
        "default-leaf-domain-number",
        "default-leaf-domain-object",
        "default-link",
        "duplicate-first-empty-leaf",
        "duplicate-query-first-wins",
        "empty-default-leaf",
        "empty-injected-still-precedes-query",
        "empty-leaf-falls-back-target-stays-empty",
        "injected-precedence",
        "no-default-leaf",
        "null-default-leaf",
        "query-link",
    ]) {
        replay(&row);
    }
}

#[test]
fn document_links_decode_names_and_values_as_form_data() {
    for row in cases(&[
        "bare-and-unknown-query-keys",
        "decoded-link-selects-literal-entry-not-decoy",
        "double-injected-question-prefix",
        "double-query-question-prefix",
        "encoded-identities",
        "encoded-query-key-and-separators",
        "injected-question-prefix",
        "malformed-percent-query",
        "query-names-case-sensitive",
    ]) {
        replay(&row);
        if row.name == "decoded-link-selects-literal-entry-not-decoy" {
            let doc = session_document_support::load(&row).unwrap();
            let maestro_app::Presence::Present(serde_json::Value::String(id)) = &doc.leaf_id else {
                panic!("missing selected identity")
            };
            assert_eq!(
                doc.data["entries"][doc.by_id[id]]["id"],
                row.output.as_ref().unwrap()["leafId"]
            );
        }
    }
}

#[test]
fn document_indexes_retain_latest_records_and_calls() {
    for row in cases(&[
        "call-values-retain-missing-null-and-unknown-arguments",
        "decoded-equal-record-identities",
        "empty-string-call-and-entry-identities",
        "entry-last-wins-occurrences-survive",
        "tool-call-last-wins-all-branches",
    ]) {
        replay(&row);
    }
}

#[test]
fn document_labels_keep_truthy_updates_without_clearing() {
    for row in cases(&[
        "false-label-target-does-not-apply",
        "label-domain-array",
        "label-domain-boolean",
        "label-domain-missing",
        "label-domain-nonempty-string",
        "label-domain-nonzero-number",
        "label-domain-null",
        "label-domain-number",
        "label-domain-object",
        "label-domain-string",
        "label-domain-true",
        "viewer-label-falsy-does-not-clear",
        "viewer-label-last-truthy-wins",
    ]) {
        replay(&row);
    }
}

#[test]
fn document_tree_keeps_root_order_and_occurrence_edges() {
    for row in cases(&[
        "nested-tree-branches",
        "orphan-self-and-missing-parent-roots",
        "same-identity-distinct-parent-occurrences",
    ]) {
        replay(&row);
    }
}

#[test]
fn document_tree_orders_timestamp_neighbors_stably() {
    for row in cases(&[
        "children-sort-root-order-stays",
        "invalid-date-comparator-neighbor",
        "offset-and-millisecond-neighbors",
    ]) {
        replay(&row);
    }
}

#[test]
fn document_tree_reuses_borrowed_records_and_terminates_rootless_cycles() {
    for row in cases(&[
        "reachable-cycle-active-path-guard",
        "unrooted-cycle-has-no-tree",
    ]) {
        replay(&row);
    }
}

#[test]
fn document_stats_count_every_branch_and_entry_category() {
    for row in cases(&[
        "empty-stats",
        "every-category-all-branches",
        "ignored-content-not-assistant",
        "unknown-and-mixed-entry-types",
    ]) {
        replay(&row);
    }
}

#[test]
fn document_stats_keep_component_usage_and_encountered_models() {
    for row in cases(&[
        "all-zero-and-negative-usage",
        "empty-model-or-provider-omitted",
        "falsy-usage-and-cost",
        "model-domain-missing",
        "model-domain-null",
        "model-domain-string",
        "numeric-domain-missing",
        "numeric-domain-null",
        "numeric-domain-number",
        "provider-domain-missing",
        "provider-domain-null",
        "provider-domain-string",
        "provider-identity-ordered-deduplication",
        "sparse-numeric-usage",
    ]) {
        replay(&row);
    }
}

#[test]
fn document_stats_accumulate_in_entry_order() {
    for row in cases(&["ordered-floating-accumulation"]) {
        replay(&row);
    }
}

#[test]
fn document_loading_reports_reached_message_failures() {
    for row in cases(&[
        "missing-assistant-content",
        "missing-message-receiver-fails-indexing",
        "nonarray-assistant-content-fails-at-stats",
        "nonarray-assistant-content-is-not-indexed",
        "null-assistant-content",
        "null-content-block",
        "null-message",
    ]) {
        replay(&row);
    }
}

#[test]
fn document_owns_outputs_after_input_is_dropped() {
    for row in cases(&["every-category-all-branches", "unicode-unknown-fields"]) {
        let document = {
            let payload = session_document_support::payload(&row.input);
            let before = payload.clone();
            let document =
                maestro_web::export_html::template::load_document(&payload, "?leafId=user", None)
                    .unwrap();
            assert_eq!(payload, before);
            document
        };
        let expected = row.output.unwrap();
        if row.input.operation.is_some() {
            session_document_support::equal(
                &session_document_support::stats(&document.global_stats),
                &expected,
                &row.name,
            );
        } else {
            assert_eq!(
                serde_json::Value::Object(document.data.clone()),
                expected["data"]
            );
            assert_eq!(
                session_document_support::indexes(&document)["byId"],
                expected["byId"]
            );
        }
        assert!(
            !session_document_support::tree(&document)
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn stats_admit_numeric_fields_without_language_coercion() {
    use maestro_web::export_html::template::compute_stats;
    use serde_json::json;
    for value in [json!("1"), json!(true), json!({"x":1}), json!([1])] {
        for key in ["input", "output", "cacheRead", "cacheWrite"] {
            for usage in [json!({key:value}), json!({"cost":{key:value}})] {
                let mut message =
                    json!({"role":"assistant","content":[],"model":"m","provider":"p","usage":{}});
                message["usage"] = usage;
                let entries = vec![json!({"type":"message","message":message})];
                assert_eq!(
                    compute_stats(&entries).unwrap_err().kind(),
                    std::io::ErrorKind::InvalidData
                );
            }
        }
    }
    for field in ["model", "provider"] {
        for value in [json!(1), json!(true), json!({}), json!([])] {
            let mut message = json!({"role":"assistant","content":[],"model":"m","provider":"p"});
            message[field] = value;
            assert_eq!(
                compute_stats(&[json!({"type":"message","message":message})])
                    .unwrap_err()
                    .kind(),
                std::io::ErrorKind::InvalidData
            );
        }
    }
    let entries = [
        json!({"type":"message","message":{"role":"assistant","content":[],"model":"m","provider":"p","usage":{"input":false,"output":"","cacheRead":null,"cacheWrite":-1}}}),
    ];
    let stats = compute_stats(&entries).unwrap();
    assert_eq!(stats.models, ["p/m"]);
    assert_eq!(stats.tokens.cache_write.to_bits(), (-1.0_f64).to_bits());
    assert_eq!(stats.tokens.input.to_bits(), 0.0_f64.to_bits());
}

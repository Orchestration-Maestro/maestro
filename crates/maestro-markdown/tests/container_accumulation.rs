#![cfg(test)]
//! Recursive container observations through each configured parser route.
#[path = "grammar_rules/observations.rs"]
mod observations;

#[test]
fn container_tables_follow_ordered_dispatch() {
    observations::check(
        "container_tables_follow_ordered_dispatch",
        5,
        include_str!("fixtures/container_accumulation.json"),
    );
}

#[test]
fn refused_tables_reenter_text_without_rule_emulation() {
    observations::check(
        "refused_tables_reenter_text_without_rule_emulation",
        3,
        include_str!("fixtures/container_accumulation.json"),
    );
}

#[test]
fn quote_chunks_preserve_setext_spelling_and_source() {
    observations::check(
        "quote_chunks_preserve_setext_spelling_and_source",
        5,
        include_str!("fixtures/container_accumulation.json"),
    );
}

#[test]
fn quote_definitions_merge_before_reference_registration() {
    observations::check(
        "quote_definitions_merge_before_reference_registration",
        1,
        include_str!("fixtures/container_accumulation.json"),
    );
}

#[test]
fn quote_code_merges_dedented_semantic_operands() {
    observations::check(
        "quote_code_merges_dedented_semantic_operands",
        2,
        include_str!("fixtures/container_accumulation.json"),
    );
}

#[test]
fn quote_reentry_uses_own_raw_and_unconsumed_remainder() {
    observations::check(
        "quote_reentry_uses_own_raw_and_unconsumed_remainder",
        3,
        include_str!("fixtures/container_accumulation.json"),
    );
}

#[test]
fn quote_consumption_advances_across_invisible_definitions() {
    observations::check(
        "quote_consumption_advances_across_invisible_definitions",
        5,
        include_str!("fixtures/container_accumulation.json"),
    );
}

#[test]
fn item_code_admission_removes_exactly_one_indent() {
    observations::check(
        "item_code_admission_removes_exactly_one_indent",
        5,
        include_str!("fixtures/container_accumulation.json"),
    );
}

#[test]
fn loose_items_keep_genuine_paragraph_indentation() {
    observations::check(
        "loose_items_keep_genuine_paragraph_indentation",
        4,
        include_str!("fixtures/container_accumulation.json"),
    );
}

#[test]
fn empty_quote_recognition_keeps_raw_consumption() {
    observations::check(
        "empty_quote_recognition_keeps_raw_consumption",
        6,
        include_str!("fixtures/container_accumulation.json"),
    );
}

#[test]
fn nested_lists_preserve_delimiters_starts_and_state() {
    observations::check(
        "nested_lists_preserve_delimiters_starts_and_state",
        2,
        include_str!("fixtures/container_accumulation.json"),
    );
}

#[test]
fn quote_chunks_restore_the_enclosing_grammar_state() {
    observations::check(
        "quote_chunks_restore_the_enclosing_grammar_state",
        1,
        include_str!("fixtures/container_accumulation.json"),
    );
}

#[test]
fn all_callers_keep_their_html_admission_policy() {
    observations::check(
        "all_callers_keep_their_html_admission_policy",
        1,
        include_str!("fixtures/container_accumulation.json"),
    );
}

#[test]
fn list_alignment_does_not_change_lazy_code_spelling() {
    observations::check(
        "list_alignment_does_not_change_lazy_code_spelling",
        4,
        include_str!("fixtures/container_accumulation.json"),
    );
}

#[test]
fn container_marker_boundaries_keep_the_real_rule_order() {
    observations::check(
        "container_marker_boundaries_keep_the_real_rule_order",
        11,
        include_str!("fixtures/container_accumulation.json"),
    );
}

#[test]
fn container_interruptions_visit_every_admitted_block() {
    observations::check(
        "container_interruptions_visit_every_admitted_block",
        6,
        include_str!("fixtures/container_accumulation.json"),
    );
}

#[test]
fn definition_continuation_adds_no_synthetic_blank() {
    observations::check(
        "definition_continuation_adds_no_synthetic_blank",
        2,
        include_str!("fixtures/container_accumulation.json"),
    );
}

#[test]
fn continued_code_keeps_authored_blank_quote_lines() {
    observations::check(
        "continued_code_keeps_authored_blank_quote_lines",
        1,
        include_str!("fixtures/container_accumulation.json"),
    );
}

#[test]
fn quote_replacement_preserves_every_unconsumed_source_character() {
    observations::check(
        "quote_replacement_preserves_every_unconsumed_source_character",
        2,
        include_str!("fixtures/container_accumulation.json"),
    );
}

#[test]
fn container_source_spelling_keeps_unicode_and_whitespace_boundaries() {
    observations::check(
        "container_source_spelling_keeps_unicode_and_whitespace_boundaries",
        5,
        include_str!("fixtures/container_accumulation.json"),
    );
}

#[test]
fn item_framing_preserves_measurement_spelling_and_history() {
    observations::check(
        "item_framing_preserves_measurement_spelling_and_history",
        13,
        include_str!("fixtures/container_accumulation.json"),
    );
}

#[test]
fn code_merges_exclude_unconsumed_indented_blank_lines() {
    use maestro_markdown::{MarkedOptions, parse_with_options};
    use serde_json::json;

    let mut failures = Vec::new();
    let inputs = [(0, 0), (4, 4), (6, 8)]
        .into_iter()
        .flat_map(|(quote, list)| {
            [
                format!("> q\n    code\n> {}\n> y", " ".repeat(quote)),
                format!("- q\n      code\n{}\n  y", " ".repeat(list)),
            ]
        });
    for source in inputs {
        for options in [
            MarkedOptions::Terminal,
            MarkedOptions::Browser { escape_html: false },
            MarkedOptions::Browser { escape_html: true },
            MarkedOptions::Export,
        ] {
            let merged = match options {
                MarkedOptions::Export => json!([["text", "q"], ["break"], ["text", "code"]]),
                _ => json!([["text", "q\ncode"]]),
            };
            let blocks = json!([
                ["paragraph", merged],
                ["gap"],
                ["paragraph", [["text", "y"]]]
            ]);
            let expected = if source.starts_with('>') {
                json!([["quote", blocks]])
            } else {
                json!([["list", null, [["item", blocks]]]])
            };
            let document = parse_with_options(source.clone(), options);
            let actual = observations::nodes(&document.nodes, true);
            if actual != expected {
                failures.push(format!("{source:?} {options:?}: {actual} != {expected}"));
            }
            assert_eq!(document.nodes[0].authored, source);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

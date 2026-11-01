#![cfg(test)]
//! Recursive container observations through each configured parser route.
use crate::observations;

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
            assert_eq!(document.nodes[0].authored.to_owned_string(), source);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn resumed_list_items_keep_admission_and_leaf_outcomes() {
    observations::check(
        "list-resumption",
        42,
        include_str!("fixtures/list_resumption.json"),
    );
}

#[test]
fn list_item_continuation_follows_the_reference_verdicts() {
    use maestro_markdown::MarkedOptions;

    let options = [
        MarkedOptions::Terminal,
        MarkedOptions::Browser { escape_html: true },
    ];
    let fixture = include_str!("fixtures/item_stops.json");
    observations::check_with_options("stop-kind", 16, fixture, &options);
    observations::check_with_options("marker-indent", 7, fixture, &options);
    observations::check_with_options("continuation-indent", 3, fixture, &options);
}

#[test]
fn item_text_first_lines_keep_the_recognizer_exemptions() {
    use maestro_markdown::parse;

    // An underline one byte into item text is not a heading, and a blank first line
    // does not close the setext search.
    let document = parse("-  \n  ==\n".to_owned());
    let maestro_markdown::Kind::List(_, items) = &document.nodes[0].kind else {
        panic!("list")
    };
    let maestro_markdown::Kind::Item(children) = &items[0].kind else {
        panic!("item")
    };
    assert!(matches!(children[0].kind, maestro_markdown::Kind::Gap));
    assert!(matches!(
        children[1].kind,
        maestro_markdown::Kind::Inline(_)
    ));
    let document = parse("-\n  a\n  ===\n".to_owned());
    let maestro_markdown::Kind::List(_, items) = &document.nodes[0].kind else {
        panic!("list")
    };
    let maestro_markdown::Kind::Item(children) = &items[0].kind else {
        panic!("item")
    };
    assert!(matches!(children[0].kind, maestro_markdown::Kind::Gap));
    assert!(matches!(
        children[1].kind,
        maestro_markdown::Kind::Heading(1, _, _)
    ));
}

#[test]
fn continuation_item_with_a_final_underline_keeps_one_heading() {
    use maestro_markdown::{Kind, parse};

    for source in [
        "- a\n  b\n  ===\n",
        "- a\n  b\n  ===\n\n",
        "- a\n  b\n  ===",
    ] {
        let document = parse(source.to_owned());
        let Kind::List(_, items) = &document.nodes[0].kind else {
            panic!("list")
        };
        let Kind::Item(children) = &items[0].kind else {
            panic!("item")
        };
        let headings: Vec<_> = children
            .iter()
            .filter(|node| matches!(node.kind, Kind::Heading(..)))
            .collect();
        assert_eq!(headings.len(), 1, "{source:?}");
        let Kind::Heading(1, _, label) = &headings[0].kind else {
            panic!("heading")
        };
        assert_eq!(label, "a\nb");
        assert_eq!(headings[0].authored.to_owned_string(), "a\nb\n===");
    }
}

#[test]
fn item_text_after_a_nested_container_keeps_the_enclosing_grammar() {
    use maestro_markdown::{Kind, parse};

    let document = parse("- a\n  - b\n  c\n".to_owned());
    let Kind::List(_, items) = &document.nodes[0].kind else {
        panic!("list")
    };
    let Kind::Item(children) = &items[0].kind else {
        panic!("item")
    };
    assert!(matches!(children[0].kind, Kind::Inline(_)));
    let Kind::List(_, inner) = &children[1].kind else {
        panic!("inner list")
    };
    let Kind::Item(body) = &inner[0].kind else {
        panic!("inner item")
    };
    assert_eq!(body.len(), 1);
    assert_eq!(body[0].authored.to_owned_string(), "b\nc");
    assert!(matches!(body[0].kind, Kind::Inline(_)));
}

#[test]
fn lazy_continuation_lines_keep_item_text_admission() {
    use maestro_markdown::{Kind, parse};

    for (source, text) in [
        ("- a\nb\n", "a\nb"),
        ("- a\n\tb\n", "a\n  b"),
        ("- a\n  b", "a\nb"),
    ] {
        let document = parse(source.to_owned());
        let Kind::List(_, items) = &document.nodes[0].kind else {
            panic!("list")
        };
        let Kind::Item(children) = &items[0].kind else {
            panic!("item")
        };
        assert_eq!(children.len(), 1, "{source:?}");
        assert_eq!(children[0].authored.to_owned_string(), text, "{source:?}");
        assert!(matches!(children[0].kind, Kind::Inline(_)));
    }
}

/// Caller block tokenizer that counts the admission steps it observes.
#[derive(Default)]
struct Counting {
    /// Grammar starts the caller was asked for.
    starts: std::cell::Cell<usize>,
    /// Tokens the caller was asked to admit.
    tokens: std::cell::Cell<usize>,
}

impl maestro_markdown::BlockTokenizer for Counting {
    fn start(&self, _source: &str) -> Option<usize> {
        self.starts.set(self.starts.get() + 1);
        None
    }

    fn tokenize(&mut self, _source: &str) -> Option<maestro_markdown::BlockToken> {
        self.tokens.set(self.tokens.get() + 1);
        None
    }
}

/// A caller tokenizer keeps the item's admission and tokenizer calls linear.
#[test]
fn item_text_with_a_caller_tokenizer_stays_linear()
-> Result<(), maestro_markdown::InvalidBlockToken> {
    use maestro_markdown::{Kind, MarkedOptions, parse_with_tokenizer};

    let mut counts = Vec::new();
    for lines in [64usize, 128] {
        let source = "- first\n".to_owned() + &"  cont\n".repeat(lines);
        let mut tokenizer = Counting::default();
        let document = parse_with_tokenizer(
            source.clone(),
            MarkedOptions::Terminal,
            Some(&mut tokenizer),
        )?;
        let Kind::List(_, items) = &document.nodes[0].kind else {
            panic!("list")
        };
        assert_eq!(items[0].authored.to_owned_string(), source);
        counts.push((tokenizer.starts.get(), tokenizer.tokens.get()));
    }
    eprintln!("caller tokenizer admissions: {counts:?}");
    assert!(counts[1].0 <= 3 * counts[0].0, "{counts:?}");
    assert!(counts[1].1 <= 3 * counts[0].1, "{counts:?}");
    Ok(())
}

//! Focused native recognition behaviors.

#[test]
fn keyword_weights_and_case_keep_exact_relevance() {
    super::fixtures::witnesses(&["keywords", "weights", "forced_weight"]);
    super::fixtures::inputs(&["sql-case", "keyword-feff", "keyword-nel", "unicode-fold"]);
}

#[test]
fn keyword_hit_cap_is_per_word_and_per_run() {
    use super::fixtures::{controlled, witnesses};
    use crate::highlight::{HighlightProfile, core::IllegalPolicy};
    witnesses(&["cap"]);
    let engine = controlled(HighlightProfile::ExportViewer);
    let cap = engine.languages.iter().find(|l| l.name == "cap").unwrap();
    for _ in 0..2 {
        let score = engine
            .evaluate(
                cap,
                &format!("{}{}", "bob ".repeat(8), "suzy ".repeat(13)),
                None,
                IllegalPolicy::Reject,
            )
            .score;
        assert_eq!(score.relevance().to_bits(), 21_f64.to_bits());
    }
}

#[test]
fn fractional_relevance_keeps_f64_evaluation_order() {
    super::fixtures::inputs(&["json-fraction"]);
}

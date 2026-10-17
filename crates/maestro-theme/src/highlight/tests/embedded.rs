//! Focused native recognition behaviors.

#[test]
fn embedded_fixed_and_subset_modes_keep_profile_admission() {
    super::fixtures::witnesses(&["autoembed", "missing"]);
    for profile in [
        crate::HighlightProfile::ExportViewer,
        crate::HighlightProfile::CodeBlock,
    ] {
        let engine = super::fixtures::controlled(profile);
        for subset in [["cap", "base"], ["base", "cap"]] {
            let result = engine.automatic("same bob", &subset.map(str::to_owned));
            assert_eq!(result[0].language, Some(subset[0]));
            assert_eq!(result[1].language, Some(subset[1]));
            assert_eq!(result[0].relevance().to_bits(), 1_f64.to_bits());
        }
    }
    super::fixtures::inputs(&[
        "xml-script",
        "xml-style",
        "template-graphql",
        "tagged-missing",
    ]);
}

#[test]
fn embedded_continuations_keep_retained_frames() {
    super::fixtures::witnesses(&["embedded"]);
    super::fixtures::inputs(&["continuation-xml", "continuation-ruby", "nested-languages"]);
}

#[test]
fn zero_host_relevance_suppresses_embedded_scores() {
    super::fixtures::witnesses(&["norelevance"]);
}

#[test]
fn nested_delimiters_keep_the_entered_frame_state() {
    super::fixtures::inputs(&[
        "nested-heredoc",
        "ruby-delimiter-alternate",
        "heredoc-wrong-end",
    ]);
}

#[test]
fn empty_illegal_embedded_modes_keep_profile_newlines() {
    super::fixtures::witnesses(&["hostillegal"]);
    super::fixtures::inputs(&["empty-illegal-embedded", "blank-eof-end"]);
}

//! Retained paragraph and underline admission at growing caller operands.
use crate::{
    MarkedOptions,
    rules::paragraph::Recognizer,
    source::{ScopeSource, Segmented},
};

#[test]
fn paragraph_chunks_wait_for_later_setext_without_revisiting_lines() {
    let source = format!("{}===\n\nnext", "alpha\nbeta\n".repeat(64));
    let mut recognizer = Recognizer::new(MarkedOptions::Terminal);
    for end in 1..=source.len() {
        let _ = recognizer.extend(&source[..end]);
    }
    let result = recognizer.finish(source.as_str());
    let heading = result.heading.unwrap();
    assert_eq!(
        &source[heading.content],
        "alpha\nbeta\n".repeat(64).trim_end_matches('\n')
    );
    assert_eq!(heading.level, 1);
    assert_eq!(&source[heading.consumed..], "next");
    assert_eq!(result.paragraph, Some("alpha\nbeta\n".repeat(64).len() + 3));
    let mut control = Recognizer::new(MarkedOptions::Terminal);
    let control = control.finish("alpha\nbeta\n\n===\n");
    assert!(control.heading.is_none());
    assert_eq!(control.paragraph, Some(10));
    let work = super::WORK.with_borrow(|work| work.paragraph_bytes);
    assert!(matches!(
        recognizer.extend("ignored"),
        crate::rules::paragraph::Verdict::Stable(_)
    ));
    assert_eq!(super::WORK.with_borrow(|work| work.paragraph_bytes), work);
}

#[test]
fn long_paragraphs_advance_without_rescanning() {
    for count in [16, 32, 64] {
        let source = ("alpha\nbeta\n".repeat(64) + "\n").repeat(count);
        let bytes = source.len();
        super::WORK.with_borrow_mut(|work| *work = super::Work::default());
        let document = crate::parse(source);
        assert_eq!(
            document
                .nodes
                .iter()
                .filter(|node| matches!(node.kind, crate::Kind::Paragraph(_)))
                .count(),
            count
        );
        let work = super::WORK.with_borrow(|work| work.paragraph_bytes);
        eprintln!("paragraphs={count}, source_bytes={bytes}, paragraph_bytes={work}");
        assert!(
            work <= 8 * bytes,
            "paragraph bytes {work} for {bytes} input bytes"
        );
    }
}

#[test]
fn adjacent_setext_headings_settle_before_later_heading_labels() {
    for count in [16, 32, 64] {
        let source = "alpha\nbeta\n===\n".repeat(count);
        let bytes = source.len();
        super::WORK.with_borrow_mut(|work| *work = super::Work::default());
        let document = crate::parse(source);
        assert_eq!(document.nodes.len(), count);
        assert!(
            document
                .nodes
                .iter()
                .all(|node| matches!(node.kind, crate::Kind::Heading(1, _, _)))
        );
        let work = super::WORK.with_borrow(|work| work.paragraph_bytes);
        assert!(
            work <= 12 * bytes,
            "heading bytes {work} for {bytes} input bytes"
        );
    }
}

/// Grammar and framing work for one continuation item of the given line count.
fn measured_item(lines: usize) -> super::Work {
    let source = "- first\n".to_owned() + &"  cont\n".repeat(lines);
    super::WORK.with_borrow_mut(|work| *work = super::Work::default());
    let document = crate::parse(source);
    assert_eq!(document.nodes.len(), 1);
    super::WORK.with_borrow(|work| *work)
}

#[test]
fn continuation_item_paragraph_work_grows_linearly() {
    let work = [64, 128, 256].map(measured_item);
    let paragraph = work.map(|work| work.paragraph_bytes);
    let framing = work.map(|work| work.framing_bytes);
    eprintln!("paragraph bytes: {paragraph:?}; framing bytes: {framing:?}");
    for pair in work.windows(2) {
        assert!(
            pair[1].paragraph_bytes <= 3 * pair[0].paragraph_bytes,
            "{paragraph:?}"
        );
        assert!(
            pair[1].framing_bytes <= 3 * pair[0].framing_bytes,
            "{framing:?}"
        );
    }
}

/// Caller grammars that select different paragraph and setext predicates.
const GRAMMARS: [MarkedOptions; 5] = [
    MarkedOptions::Terminal,
    MarkedOptions::Browser { escape_html: false },
    MarkedOptions::Documentation,
    MarkedOptions::Browser { escape_html: true },
    MarkedOptions::Export,
];

/// Heading and undecidedness a fresh recognizer reaches on one operand.
fn fresh(
    options: MarkedOptions,
    operand: &str,
) -> (Option<crate::rules::paragraph::Heading>, bool) {
    let mut recognizer = Recognizer::new(options);
    let undecided = matches!(
        recognizer.extend(operand),
        crate::rules::paragraph::Verdict::NeedMore
    );
    (recognizer.finish(operand).heading, undecided)
}

/// Every line offset of one operand, in the order the item path reads them.
fn line_offsets(operand: &str) -> Vec<usize> {
    let mut offsets = vec![0];
    offsets.extend(operand.match_indices('\n').map(|(index, _)| index + 1));
    offsets
}

/// Compare one retained lookahead with a fresh recognizer at every line offset.
fn compare_retained(options: MarkedOptions, operand: &str) {
    let mut text = crate::rules::paragraph::ItemText::new(options);
    for start in line_offsets(operand) {
        let suffix = Segmented::from(ScopeSource::identity(&operand[start..]));
        let observed = text.attempt(&suffix);
        let expected = fresh(options, &operand[start..]);
        assert_eq!(observed, expected, "{options:?} at {start} of {operand:?}");
    }
}

#[test]
fn retained_item_text_answers_match_a_fresh_recognizer() {
    let operand = "first\nsecond\n===\n\nthird\n- x\n|a|b|\n|---|---|\n> q\nunder\n";
    for options in GRAMMARS {
        compare_retained(options, operand);
    }
}

#[test]
fn retained_item_text_answers_match_on_generated_operands() {
    let parts = [
        "==\n",
        "---\n",
        "\n",
        "```\n",
        "# h\n",
        "- x\n",
        "> q\n",
        "|a|b|\n",
        "|---|---|\n",
        "<div>\n",
        "text\r\n",
        "\u{2028}x\n",
        "a",
    ];
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    for _ in 0..3_000 {
        let mut operand = String::new();
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        let lines = 1 + (state >> 33) as usize % 12;
        for _ in 0..lines {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            operand.push_str(parts[(state >> 33) as usize % parts.len()]);
        }
        for options in GRAMMARS {
            compare_retained(options, &operand);
        }
    }
}

#[test]
fn item_text_stops_follow_every_recognizer_predicate() {
    let predicates = [
        "\n",
        "# h\n",
        "```\n",
        "|---|\n",
        "<div>\n",
        "> q\n",
        "- x\n",
        "x\r\n",
        "\u{2028}x\n",
    ];
    for predicate in predicates {
        let closed = format!("a\n{predicate}===\n");
        let reopened = format!("a\n{predicate}b\n===\n");
        let after = 2 + predicate.len();
        let mut closers = 0;
        for options in GRAMMARS {
            compare_retained(options, &closed);
            compare_retained(options, &reopened);
            if fresh(options, &closed).0.is_none() {
                closers += 1;
            }
            let (heading, _) = fresh(options, &reopened[after..]);
            assert!(heading.is_some(), "{options:?} lost {reopened:?}");
        }
        assert!(closers > 0, "no caller grammar closes on {predicate:?}");
        for options in GRAMMARS {
            compare_retained(options, "a\n===\n");
        }
    }
}
